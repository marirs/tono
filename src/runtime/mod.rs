//! Private, versioned runtime installation shared by CLI and future GUI.
//! Call `ensure_ready` on a background thread; progress never requires stdin.
mod install;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

include!(concat!(env!("OUT_DIR"), "/assets.rs"));
const MANIFEST: &str = include_str!(concat!(env!("OUT_DIR"), "/runtime-manifest.json"));
pub const TARGET: &str = env!("TONO_TARGET");

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RuntimeManifest {
    pub version: String,
    pub target: String,
    pub url: String,
    pub sha256: String,
    pub bytes: u64,
    pub unpacked_bytes: u64,
}

impl RuntimeManifest {
    fn validate(&self) -> Result<()> {
        if self.version != env!("CARGO_PKG_VERSION") || self.target != TARGET {
            bail!("runtime does not match this Tono version/platform");
        }
        if self.sha256.len() != 64
            || !self.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || !self.url.starts_with("https://")
            || self.bytes == 0
            || self.unpacked_bytes == 0
        {
            bail!("invalid release runtime manifest");
        }
        Ok(())
    }
    pub fn archive_name(&self) -> String {
        format!("tono-runtime-{}.tar.gz", self.target)
    }
}

#[derive(Debug, Clone)]
pub enum SetupProgress {
    Waiting,
    UsingOfflineArchive,
    Downloading { received: u64, total: u64 },
    Verifying,
    Extracting,
    Checking,
    Ready(PathBuf),
}

pub fn release_manifest() -> Result<Option<RuntimeManifest>> {
    if MANIFEST.is_empty() {
        return Ok(None);
    }
    let manifest: RuntimeManifest = serde_json::from_str(MANIFEST)?;
    manifest.validate()?;
    Ok(Some(manifest))
}

pub fn data_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("TONO_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }
    Ok(directories::ProjectDirs::from("", "", "tono")
        .context("cannot locate Tono's user-data directory")?
        .data_local_dir()
        .to_owned())
}

fn location(manifest: &RuntimeManifest) -> Result<PathBuf> {
    Ok(data_dir()?.join("runtimes").join(&manifest.sha256))
}

pub(crate) fn release_root() -> Option<PathBuf> {
    location(&release_manifest().ok()??).ok()
}

pub fn installed_root() -> Option<PathBuf> {
    let manifest = release_manifest().ok()??;
    let path = location(&manifest).ok()?;
    install::is_ready(&path, &manifest).then_some(path)
}

/// An adjacent archive permits fully offline first use. Online releases fetch
/// exactly the archive whose checksum was embedded by the release build.
/// Source builds retain the existing developer environment without downloads.
pub fn ensure_ready(mut progress: impl FnMut(SetupProgress)) -> Result<()> {
    let Some(manifest) = release_manifest()? else {
        return Ok(());
    };
    let archive = std::env::current_exe()?
        .parent()
        .context("executable has no directory")?
        .join(manifest.archive_name());
    install::install(
        &manifest,
        &location(&manifest)?,
        archive.is_file().then_some(archive.as_path()),
        &mut progress,
    )?;
    Ok(())
}

pub fn ensure_ready_cli() -> Result<()> {
    let mut last_percent = None;
    let mut offline = false;
    ensure_ready(|event| match event {
        SetupProgress::Waiting => eprintln!("Preparing Tono for first use…"),
        SetupProgress::UsingOfflineArchive => {
            offline = true;
            eprintln!("Using the included offline runtime…");
        }
        SetupProgress::Downloading { received, total } => {
            let percent = (received.saturating_mul(100) / total.max(1)).min(100) / 5 * 5;
            if last_percent != Some(percent) {
                if offline {
                    eprintln!("Reading offline runtime… {percent}%");
                } else {
                    eprintln!("Downloading audio and ML runtime… {percent}%");
                }
                last_percent = Some(percent);
            }
        }
        SetupProgress::Verifying => eprintln!("Verifying runtime…"),
        SetupProgress::Extracting => eprintln!("Installing private audio and ML tools…"),
        SetupProgress::Checking => eprintln!("Checking installed tools and models…"),
        SetupProgress::Ready(_) => eprintln!("✓ Tono runtime ready\n"),
    })
}

pub(crate) fn python_at(root: &Path) -> PathBuf {
    root.join(if cfg!(windows) {
        "python.exe"
    } else {
        "bin/python"
    })
}

pub(crate) fn tool_at(root: &Path, name: &str) -> PathBuf {
    root.join(if cfg!(windows) { "Library/bin" } else { "bin" })
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

/// Configure only the subprocess, never mutate the GUI/host process environment.
pub fn configure_command(command: &mut Command, root: &Path) -> Result<()> {
    let mut path = vec![
        root.join("bin"),
        root.join("Library/bin"),
        root.to_owned(),
        root.join("Scripts"),
    ];
    path.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    command
        .env("PATH", std::env::join_paths(path)?)
        .env("HOME", root.join("home"))
        .env("USERPROFILE", root.join("home"))
        .env("HF_HOME", root.join("models/huggingface"))
        .env("TONO_PANNS_DIR", root.join("models/panns"))
        .env("HF_HUB_OFFLINE", "1")
        .env("PYTHONNOUSERSITE", "1")
        .env_remove("PYTHONHOME")
        .env_remove("PYTHONPATH");
    Ok(())
}

pub(crate) fn check(root: &Path) -> Result<()> {
    let python = python_at(root);
    let mut command = Command::new(&python);
    configure_command(&mut command, root)?;
    let result = command
        .arg(root.join("ml/analyze.py"))
        .arg("--self-check")
        .output()?;
    if !result.status.success() {
        bail!(
            "runtime self-check failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let report: serde_json::Value = serde_json::from_slice(&result.stdout)?;
    for section in ["modules", "models"] {
        let checks = report[section]
            .as_object()
            .context("invalid runtime self-check report")?;
        if checks.is_empty() || checks.values().any(|v| v["ok"] != true) {
            bail!("runtime {section} check failed: {}", report[section]);
        }
    }
    for name in ["ffmpeg", "ffprobe"] {
        let mut command = Command::new(tool_at(root, name));
        configure_command(&mut command, root)?;
        let result = command.arg("-version").output()?;
        if !result.status.success() {
            bail!("bundled {name} is not runnable");
        }
    }
    let mut command = Command::new(tool_at(root, "ffmpeg"));
    configure_command(&mut command, root)?;
    let result = command.args(["-hide_banner", "-encoders"]).output()?;
    let encoders = String::from_utf8_lossy(&result.stdout);
    if !result.status.success() || !encoders.contains("libx264") || !encoders.contains(" aac ") {
        bail!("bundled ffmpeg needs libx264 and AAC encoders");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_rejects_wrong_platform_version_or_untrusted_transport() {
        let mut m = RuntimeManifest {
            version: env!("CARGO_PKG_VERSION").into(),
            target: TARGET.into(),
            url: "https://example.com/runtime.tar.gz".into(),
            sha256: "a".repeat(64),
            bytes: 1,
            unpacked_bytes: 1,
        };
        m.validate().unwrap();
        m.target = "other".into();
        assert!(m.validate().is_err());
        m.target = TARGET.into();
        m.version = "other".into();
        assert!(m.validate().is_err());
        m.version = env!("CARGO_PKG_VERSION").into();
        m.url = "http://example.com/runtime.tar.gz".into();
        assert!(m.validate().is_err());
    }
}
