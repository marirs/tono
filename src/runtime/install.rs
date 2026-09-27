use super::{RuntimeManifest, SetupProgress};
use anyhow::{bail, Context, Result};
use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path},
    process::Command,
    time::Duration,
};

pub(super) fn is_ready(root: &Path, manifest: &RuntimeManifest) -> bool {
    fs::read_to_string(root.join(".tono-ready")).ok().as_deref() == Some(manifest.sha256.as_str())
        && super::python_at(root).is_file()
        && root.join("ml/analyze.py").is_file()
        && super::tool_at(root, "ffmpeg").is_file()
        && super::tool_at(root, "ffprobe").is_file()
}

pub(super) fn install(
    manifest: &RuntimeManifest,
    root: &Path,
    archive: Option<&Path>,
    progress: &mut impl FnMut(SetupProgress),
) -> Result<()> {
    install_checked(manifest, root, archive, progress, |root| {
        relocate(root)?;
        super::check(root)
    })
}

fn install_checked(
    manifest: &RuntimeManifest,
    root: &Path,
    archive: Option<&Path>,
    progress: &mut impl FnMut(SetupProgress),
    check: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    manifest.validate()?;
    if is_ready(root, manifest) {
        return Ok(());
    }
    progress(SetupProgress::Waiting);
    let parent = root.parent().context("runtime path needs parent")?;
    fs::create_dir_all(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(parent.join("setup.lock"))?;
    lock.lock_exclusive()?; // OS releases the lock on crash; a second process rechecks.
    if is_ready(root, manifest) {
        return Ok(());
    }
    let mut download = tempfile::NamedTempFile::new_in(parent)?;
    if let Some(archive) = archive {
        progress(SetupProgress::UsingOfflineArchive);
        // Copy before verifying so another process cannot replace the file
        // between verification and extraction.
        copy_verified(
            File::open(archive)?,
            download.as_file_mut(),
            manifest,
            progress,
        )?;
    } else {
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(3600))
            .user_agent(concat!("tono/", env!("CARGO_PKG_VERSION")))
            .build()?;
        let response = client
            .get(&manifest.url)
            .send()?
            .error_for_status()
            .context(
            "runtime download failed; check your connection and rerun (or use the offline bundle)",
        )?;
        copy_verified(response, download.as_file_mut(), manifest, progress)?;
    }
    progress(SetupProgress::Extracting);
    let staging = tempfile::Builder::new()
        .prefix("install-")
        .tempdir_in(parent)?;
    unpack(download.path(), staging.path(), manifest.unpacked_bytes)?;
    make_writable(staging.path())?;
    // The binary owns scripts/profiles; they cannot drift from its worker contract.
    for (name, bytes) in super::ASSETS {
        let path = staging.path().join(name);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, bytes)?;
    }
    // Conda relocation must happen at its final pathname. No ready marker is
    // visible until relocation and self-check succeed. Interrupted installs retry.
    if root.exists() {
        fs::remove_dir_all(root)?;
    }
    fs::rename(staging.path(), root)?;
    let result = (|| -> Result<()> {
        progress(SetupProgress::Checking);
        check(root)?;
        fs::write(root.join(".tono-ready"), &manifest.sha256)?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_dir_all(root);
        return Err(error.context("runtime setup failed; rerun to retry"));
    }
    progress(SetupProgress::Ready(root.to_owned()));
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn copy_verified(
    mut input: impl Read,
    output: &mut impl Write,
    manifest: &RuntimeManifest,
    progress: &mut impl FnMut(SetupProgress),
) -> Result<()> {
    let mut hash = Sha256::new();
    let mut received = 0u64;
    let mut buffer = [0u8; 128 * 1024];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        received += count as u64;
        if received > manifest.bytes {
            bail!("runtime archive exceeds expected size");
        }
        hash.update(&buffer[..count]);
        output.write_all(&buffer[..count])?;
        progress(SetupProgress::Downloading {
            received,
            total: manifest.bytes,
        });
    }
    progress(SetupProgress::Verifying);
    if received != manifest.bytes || hex(&hash.finalize()) != manifest.sha256.to_lowercase() {
        bail!("runtime checksum/size mismatch; nothing was installed, please retry");
    }
    output.flush()?;
    Ok(())
}

fn safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

fn safe_link(entry: &Path, link: &Path, hard: bool) -> bool {
    let mut depth = if hard {
        0
    } else {
        entry
            .parent()
            .map(|p| {
                p.components()
                    .filter(|c| matches!(c, Component::Normal(_)))
                    .count()
            })
            .unwrap_or(0)
    };
    for component in link.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir if depth > 0 => depth -= 1,
            _ => return false,
        }
    }
    !link.as_os_str().is_empty()
}

fn unpack(archive: &Path, destination: &Path, max_bytes: u64) -> Result<()> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(File::open(archive)?));
    let mut total = 0u64;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        if !safe_relative(&path) {
            bail!("unsafe path in runtime archive");
        }
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir() || kind.is_symlink() || kind.is_hard_link()) {
            bail!("unsupported runtime archive entry");
        }
        if let Some(link) = entry.link_name()? {
            if !safe_link(&path, &link, kind.is_hard_link()) {
                bail!("unsafe link in runtime archive");
            }
        }
        total = total
            .checked_add(entry.size())
            .context("runtime archive too large")?;
        if total > max_bytes {
            bail!("runtime archive exceeds unpacked size limit");
        }
        if !entry.unpack_in(destination)? {
            bail!("runtime entry escaped install directory");
        }
    }
    Ok(())
}

// Conda relocation updates embedded prefixes in package/cache metadata. Some
// upstream model caches deliberately mark files read-only; this private copy
// must be writable by its owner. Never follow archive symlinks here.
fn make_writable(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    let mut permissions = meta.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(permissions.mode() | if meta.is_dir() { 0o700 } else { 0o600 });
    }
    #[cfg(windows)]
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions)?;
    if meta.is_dir() {
        for entry in fs::read_dir(path)? {
            make_writable(&entry?.path())?;
        }
    }
    Ok(())
}

fn relocate(root: &Path) -> Result<()> {
    let mut command = if cfg!(windows) {
        Command::new(root.join("Scripts/conda-unpack.exe"))
    } else {
        let mut command = Command::new(super::python_at(root));
        command.arg(root.join("bin/conda-unpack"));
        command
    };
    super::configure_command(&mut command, root)?;
    let output = command.output().context("relocating bundled Python")?;
    if !output.status.success() {
        bail!(
            "Python relocation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(parent: &Path) -> (std::path::PathBuf, RuntimeManifest) {
        let path = parent.join("runtime.tar.gz");
        let gz = flate2::write::GzEncoder::new(
            File::create(&path).unwrap(),
            flate2::Compression::fast(),
        );
        let mut builder = tar::Builder::new(gz);
        let mut header = tar::Header::new_gnu();
        header.set_size(5);
        header.set_mode(0o444);
        header.set_cksum();
        builder
            .append_data(&mut header, "payload.txt", &b"hello"[..])
            .unwrap();
        builder.into_inner().unwrap().finish().unwrap();
        let data = fs::read(&path).unwrap();
        let m = RuntimeManifest {
            version: env!("CARGO_PKG_VERSION").into(),
            target: super::super::TARGET.into(),
            url: "https://example.com/runtime".into(),
            sha256: hex(&Sha256::digest(&data)),
            bytes: data.len() as u64,
            unpacked_bytes: 5,
        };
        (path, m)
    }

    #[test]
    fn failed_setup_is_retryable_and_success_is_reused_offline() {
        let temp = tempfile::tempdir().unwrap();
        let (archive, manifest) = fixture(temp.path());
        let root = temp.path().join("runtimes").join(&manifest.sha256);
        assert!(
            install_checked(&manifest, &root, Some(&archive), &mut |_| {}, |_| bail!(
                "broken runtime"
            ))
            .is_err()
        );
        assert!(!root.exists());
        // Simulate an interrupted install without a marker.
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("partial"), "bad").unwrap();
        install_checked(&manifest, &root, Some(&archive), &mut |_| {}, |root| {
            assert!(!root.join("partial").exists());
            assert!(!is_ready(root, &manifest));
            assert_eq!(fs::read(root.join("payload.txt"))?, b"hello");
            fs::write(root.join("payload.txt"), "relocated")?;
            for path in [
                super::super::python_at(root),
                super::super::tool_at(root, "ffmpeg"),
                super::super::tool_at(root, "ffprobe"),
            ] {
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(path, "test")?;
            }
            Ok(())
        })
        .unwrap();
        assert!(is_ready(&root, &manifest));
        fs::remove_file(archive).unwrap();
        install_checked(
            &manifest,
            &root,
            None,
            &mut |_| panic!("no setup needed"),
            |_| panic!("must reuse"),
        )
        .unwrap();
    }

    #[test]
    fn unpacked_size_is_bounded_before_writing_payload() {
        let temp = tempfile::tempdir().unwrap();
        let (archive, _) = fixture(temp.path());
        let destination = temp.path().join("extract");
        fs::create_dir(&destination).unwrap();
        assert!(unpack(&archive, &destination, 4).is_err());
        assert!(!destination.join("payload.txt").exists());
    }

    #[test]
    fn checksum_and_size_are_verified_before_installation() {
        let data = b"runtime";
        let mut m = RuntimeManifest {
            version: env!("CARGO_PKG_VERSION").into(),
            target: super::super::TARGET.into(),
            url: "https://example.com/r".into(),
            sha256: hex(&Sha256::digest(data)),
            bytes: data.len() as u64,
            unpacked_bytes: 100,
        };
        copy_verified(&data[..], &mut Vec::new(), &m, &mut |_| {}).unwrap();
        assert!(copy_verified(&b"corrupt"[..], &mut Vec::new(), &m, &mut |_| {}).is_err());
        m.bytes -= 1;
        assert!(copy_verified(&data[..], &mut Vec::new(), &m, &mut |_| {}).is_err());
    }
    #[test]
    fn archives_cannot_escape_through_paths_or_links() {
        assert!(!safe_relative(Path::new("../outside")));
        assert!(!safe_relative(Path::new("/outside")));
        assert!(!safe_link(
            Path::new("bin/python"),
            Path::new("../../outside"),
            false
        ));
        assert!(!safe_link(
            Path::new("bin/python"),
            Path::new("../lib/python"),
            true
        ));
        assert!(safe_link(
            Path::new("bin/python"),
            Path::new("../lib/python"),
            false
        ));
    }
}
