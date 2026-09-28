//! Naming and publishing direct-command outputs after successful preparation.
use crate::{
    instruments::Instrument,
    pipeline::prep::{self, PrepOptions},
};
use anyhow::{bail, Context, Result};
#[cfg(unix)]
use std::os::unix::fs::DirBuilderExt;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn local_date() -> Result<String> {
    Ok(chrono::Local::now().format("%Y%m%d").to_string())
}

pub fn automatic_path(
    input: &Path,
    instrument: Instrument,
    directory: Option<&Path>,
    date: &str,
) -> Result<PathBuf> {
    let mut filename = input
        .file_stem()
        .context("input needs a filename")?
        .to_os_string();
    filename.push(format!("_{}_{date}.mp4", instrument.id()));
    Ok(directory
        .unwrap_or_else(|| Path::new("./tono-practices"))
        .join(filename))
}

pub fn automatic_path_for_mode(
    input: &Path,
    instrument: Instrument,
    mode: Option<crate::instruments::brisa::FingeringMode>,
    directory: Option<&Path>,
    date: &str,
) -> Result<PathBuf> {
    crate::instruments::brisa::validate_mode(instrument, mode)?;
    if let Some(mode) = mode {
        let mut filename = input
            .file_stem()
            .context("input needs a filename")?
            .to_os_string();
        filename.push(format!("_{}_{}_{date}.mp4", instrument.id(), mode.id()));
        Ok(directory
            .unwrap_or_else(|| Path::new("./tono-practices"))
            .join(filename))
    } else {
        automatic_path(input, instrument, directory, date)
    }
}

pub fn validate_destination(input: &Path, video: &Path) -> Result<()> {
    let project = video
        .parent()
        .context("video needs a practice directory")?
        .to_path_buf();
    for (path, directory) in [(video, false), (project.as_path(), true)] {
        match fs::symlink_metadata(path) {
            Ok(meta)
                if meta.file_type().is_symlink()
                    || (directory && !meta.is_dir())
                    || (!directory && !meta.is_file()) =>
            {
                bail!(
                    "output destination has an unexpected file type: {}",
                    path.display()
                );
            }
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.into()),
            _ => {}
        }
    }
    if input == video {
        bail!("output must not overwrite the input file");
    }
    if input.exists() && video.exists() && same_file::is_same_file(input, video)? {
        bail!("output must not overwrite the input file");
    }
    if let (Ok(source), Ok(directory)) = (input.canonicalize(), project.canonicalize()) {
        if source.starts_with(directory) {
            bail!("input cannot be inside the supporting-files directory being replaced");
        }
    }
    Ok(())
}

fn private_directory(parent: &Path, prefix: &str) -> Result<PathBuf> {
    fs::create_dir_all(parent)?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let path = parent.join(format!("{prefix}-{}-{nonce}", std::process::id()));
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    builder.mode(0o700);
    builder.create(&path)?;
    Ok(path)
}

fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(source)?;
    if meta.file_type().is_symlink() {
        #[cfg(unix)]
        std::os::unix::fs::symlink(fs::read_link(source)?, target)?;
        #[cfg(windows)]
        if source.is_dir() {
            std::os::windows::fs::symlink_dir(fs::read_link(source)?, target)?;
        } else {
            std::os::windows::fs::symlink_file(fs::read_link(source)?, target)?;
        }
    } else if meta.is_dir() {
        fs::create_dir_all(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_tree(&entry.path(), &target.join(entry.file_name()))?;
        }
    } else {
        fs::copy(source, target)?;
    }
    Ok(())
}

fn remove(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path)?,
        Ok(_) => fs::remove_file(path)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

/// Back up the complete practice folder; restore it on a publication failure.
fn publish(stage: &Path, video: &Path, backup: &Path) -> Result<()> {
    let project = video
        .parent()
        .context("video needs a practice directory")?
        .to_path_buf();
    let previous_project = project.exists();
    if previous_project {
        copy_tree(&project, &backup.join("previous-practice"))?;
    }
    let result = (|| -> Result<()> {
        if let Some(parent) = video.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        remove(&project)?;
        copy_tree(&stage.join("project"), &project)?;
        fs::copy(stage.join("practice.mp4"), video)?;
        Ok(())
    })();
    if let Err(error) = result {
        let restore = (|| -> Result<()> {
            remove(&project)?;
            if previous_project {
                copy_tree(&backup.join("previous-practice"), &project)?;
            }
            Ok(())
        })();
        restore.with_context(|| {
            format!(
                "restoring previous outputs failed; recovery files: {}",
                backup.display()
            )
        })?;
        return Err(error.context("publishing failed; previous outputs restored"));
    }
    Ok(())
}

pub fn run(options: &PrepOptions) -> Result<()> {
    let video = options
        .output_video
        .as_deref()
        .context("missing MP4 output")?;
    validate_destination(&options.input, video)?;
    for source in options
        .import
        .backing
        .iter()
        .chain(options.import.audio.iter())
    {
        validate_destination(source, video)?;
    }
    let stage = private_directory(&std::env::temp_dir(), "tono-render")?;
    let mut staged = options.clone();
    staged.output_directory = stage.join("project");
    staged.output_video = Some(stage.join("practice.mp4"));
    if let Err(error) = prep::run_prep_inner(&staged, false) {
        if staged.output_directory.exists() {
            eprintln!(
                "Previous outputs unchanged. Diagnostics: {}",
                stage.display()
            );
        } else {
            fs::remove_dir_all(&stage)?;
        }
        return Err(error);
    }
    // Every generated file reference stays portable within the practice folder.
    let project_path = staged.output_directory.join("project.json");
    let mut project: serde_json::Value = serde_json::from_slice(&fs::read(&project_path)?)?;
    project["practice"]["file"] = serde_json::json!("practice.mp4");
    fs::write(project_path, serde_json::to_string_pretty(&project)? + "\n")?;
    validate_destination(&options.input, video)?;
    for source in options
        .import
        .backing
        .iter()
        .chain(options.import.audio.iter())
    {
        validate_destination(source, video)?;
    }
    let replacing = video.exists() || options.output_directory.exists();
    let backup = if replacing {
        let home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".codex")))
            .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().join(".codex")))
            .context("cannot locate recovery directory")?;
        private_directory(&home.join("artifacts/tono"), "previous-practice")?
    } else {
        private_directory(&stage, "backup")?
    };
    super::sheet::set_video_link(&staged.output_directory, Path::new("practice.mp4"))?;
    publish(&stage, video, &backup)
        .with_context(|| format!("new render retained at {}", stage.display()))?;
    fs::remove_dir_all(&stage)?;
    println!("\n✓ Ready: {}", video.display());
    println!("  Supporting files: {}", options.output_directory.display());
    println!(
        "  Review fingerings, then play: {}",
        options.output_directory.join("practice.html").display()
    );
    if replacing {
        println!("  Previous output backup: {}", backup.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_use_source_instrument_date_and_directory() {
        assert_eq!(
            automatic_path(
                Path::new("/music/PooveSempoove.mp3"),
                Instrument::Ae20,
                None,
                "20260927"
            )
            .unwrap(),
            Path::new("./tono-practices/PooveSempoove_ae20_20260927.mp4")
        );
        assert_eq!(
            automatic_path(
                Path::new("/music/my song.v2.MOV"),
                Instrument::Guitar,
                Some(Path::new("/custom")),
                "20260928"
            )
            .unwrap(),
            Path::new("/custom/my song.v2_guitar_20260928.mp4")
        );
    }
    #[test]
    fn protects_source_aliases_and_sources_inside_project() {
        let root = private_directory(&std::env::temp_dir(), "tono-output-test").unwrap();
        let input = root.join("input.mp4");
        let video = root.join("output/practice.mp4");
        fs::create_dir_all(video.parent().unwrap()).unwrap();
        fs::write(&input, "source").unwrap();
        fs::hard_link(&input, &video).unwrap();
        assert!(validate_destination(&input, &video).is_err());
        fs::remove_file(&video).unwrap();

        let nested = video.parent().unwrap().to_path_buf().join("source.mp3");
        fs::write(&nested, "source").unwrap();
        assert!(validate_destination(&nested, &video).is_err());
        assert!(validate_destination(&input, &video).is_ok());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn replacement_and_failed_publication_preserve_recovery_data() {
        let root = private_directory(&std::env::temp_dir(), "tono-publish-test").unwrap();
        let stage = root.join("stage");
        fs::create_dir_all(stage.join("project")).unwrap();
        fs::write(stage.join("project/notes.json"), "new notes").unwrap();
        let video = root.join("output/practice.mp4");
        fs::create_dir_all(video.parent().unwrap()).unwrap();
        fs::write(&video, "old video").unwrap();

        fs::write(
            video.parent().unwrap().to_path_buf().join("user.txt"),
            "old data",
        )
        .unwrap();
        let backup = private_directory(&root, "backup").unwrap();
        // Missing staged MP4 forces failure after the project was copied.
        assert!(publish(&stage, &video, &backup).is_err());
        assert_eq!(fs::read_to_string(&video).unwrap(), "old video");
        assert_eq!(
            fs::read_to_string(video.parent().unwrap().to_path_buf().join("user.txt")).unwrap(),
            "old data"
        );
        fs::write(stage.join("practice.mp4"), "new video").unwrap();
        let backup = private_directory(&root, "backup").unwrap();
        publish(&stage, &video, &backup).unwrap();
        assert_eq!(fs::read_to_string(&video).unwrap(), "new video");
        assert_eq!(
            fs::read_to_string(video.parent().unwrap().to_path_buf().join("notes.json")).unwrap(),
            "new notes"
        );
        assert_eq!(
            fs::read_to_string(backup.join("previous-practice/user.txt")).unwrap(),
            "old data"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
