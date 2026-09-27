//! Locating the Tono project directory and external executables.

use std::env;
use std::path::{Path, PathBuf};

/// Root containing `instruments/` and `ml/`.
///
/// v0 is a private prototype run from the source checkout, so we default to
/// the crate directory baked in at compile time. `TONO_ROOT` overrides it for
/// a relocated checkout.
pub fn project_root() -> PathBuf {
    env::var_os("TONO_ROOT")
        .map(PathBuf::from)
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
    let path_directories = env::var_os("PATH")
        .map(|raw| env::split_paths(&raw).collect::<Vec<_>>())
        .unwrap_or_default();
    path_directories
        .into_iter()
        .chain(FALLBACK_BINARY_DIRECTORIES.iter().map(PathBuf::from))
        .map(|directory| directory.join(name))
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
    let venv_python = ml_virtualenv_dir().join("bin/python");
    if is_executable_file(&venv_python) {
        return Some(venv_python);
    }
    find_executable("python3", "TONO_PYTHON")
}

pub fn ml_virtualenv_dir() -> PathBuf {
    project_root().join("ml/.venv")
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
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
