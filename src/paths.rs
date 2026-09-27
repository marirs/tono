//! Locating the Tono project directory and external executables.

use std::env;
use std::path::{Path, PathBuf};

/// Root containing `instruments/` and `ml/`.
///
/// Release builds use their private runtime. Source builds retain the checkout;
/// `TONO_ROOT` explicitly selects a developer/custom resource directory.
pub fn project_root() -> PathBuf {
    env::var_os("TONO_ROOT")
        .map(PathBuf::from)
        .or_else(crate::runtime::release_root)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

/// Homebrew prefixes are searched after `PATH` because shells launched by
/// editors/agents on macOS frequently lack `/opt/homebrew/bin`.
const FALLBACK_BINARY_DIRECTORIES: [&str; 2] = ["/opt/homebrew/bin", "/usr/local/bin"];

/// Resolves an executable: explicit env override first, then `PATH`, then
/// the Homebrew fallbacks. Returns `None` if nothing executable is found.
pub fn find_executable(name: &str, override_env_var: &str) -> Option<PathBuf> {
    if let Some(explicit) = env::var_os(override_env_var) {
        let explicit_path = PathBuf::from(explicit);
        return is_executable_file(&explicit_path).then_some(explicit_path);
    }
    if let Some(root) = crate::runtime::installed_root() {
        let path = crate::runtime::tool_at(&root, name);
        if is_executable_file(&path) {
            return Some(path);
        }
    }
    if crate::runtime::release_manifest().ok().flatten().is_some() {
        return None;
    }
    let path_directories = env::var_os("PATH")
        .map(|raw| env::split_paths(&raw).collect::<Vec<_>>())
        .unwrap_or_default();
    path_directories
        .into_iter()
        .chain(FALLBACK_BINARY_DIRECTORIES.iter().map(PathBuf::from))
        .map(|directory| directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)))
        .find(|candidate| is_executable_file(candidate))
}

pub fn ffmpeg_executable() -> Option<PathBuf> {
    find_executable("ffmpeg", "TONO_FFMPEG")
}

pub fn ffprobe_executable() -> Option<PathBuf> {
    find_executable("ffprobe", "TONO_FFPROBE")
}

/// Python used for the ML worker. Preference order: `TONO_PYTHON`, the
/// project virtualenv at `ml/.venv`, then `python3` on `PATH`.
pub fn python_executable() -> Option<PathBuf> {
    if env::var_os("TONO_PYTHON").is_some() {
        return find_executable("python3", "TONO_PYTHON");
    }
    if let Some(root) = crate::runtime::installed_root() {
        return Some(crate::runtime::python_at(&root));
    }
    if crate::runtime::release_manifest().ok().flatten().is_some() {
        return None;
    }
    let venv_python = ml_environment_python();
    if is_executable_file(&venv_python) {
        return Some(venv_python);
    }
    find_executable("python3", "TONO_PYTHON")
}

pub fn ml_virtualenv_dir() -> PathBuf {
    crate::runtime::installed_root().unwrap_or_else(|| project_root().join("ml/.venv"))
}

/// Interpreter inside the managed runtime or development virtualenv.
/// Windows conda bundles use python.exe at root; venv uses Scripts/python.exe.
pub fn ml_environment_python() -> PathBuf {
    if let Some(root) = crate::runtime::installed_root() {
        environment_python_at(&root, true, cfg!(windows))
    } else {
        environment_python_at(&ml_virtualenv_dir(), false, cfg!(windows))
    }
}

fn environment_python_at(root: &Path, packaged: bool, windows: bool) -> PathBuf {
    root.join(match (windows, packaged) {
        (true, true) => "python.exe",
        (true, false) => "Scripts/python.exe",
        (false, _) => "bin/python",
    })
}

#[cfg(test)]
mod environment_tests {
    use super::*;

    #[test]
    fn managed_and_development_python_layouts() {
        let root = Path::new("runtime");
        assert_eq!(
            environment_python_at(root, true, true),
            root.join("python.exe")
        );
        assert_eq!(
            environment_python_at(root, false, true),
            root.join("Scripts/python.exe")
        );
        for packaged in [true, false] {
            assert_eq!(
                environment_python_at(root, packaged, false),
                root.join("bin/python")
            );
        }
    }
}

/// The researched AE-01 fingering table (spec "AE-01 fingering").
#[cfg(test)]
pub fn ae01_fingering_table() -> PathBuf {
    crate::instruments::Instrument::Ae01.profile_path()
}

pub fn ml_worker_script() -> PathBuf {
    project_root().join("ml/analyze.py")
}

fn is_executable_file(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return std::fs::metadata(path)
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

pub fn python_command() -> anyhow::Result<std::process::Command> {
    use anyhow::Context;
    let mut command = std::process::Command::new(
        python_executable().context("Python unavailable; run tono setup")?,
    );
    if let Some(root) = crate::runtime::installed_root() {
        crate::runtime::configure_command(&mut command, &root)?;
    }
    Ok(command)
}
