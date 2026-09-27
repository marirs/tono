//! `tono doctor`: environment checks (milestone M0).
//!
//! Hard requirements fail the command; ML packages are reported as warnings
//! until M2 because M1 (hard-coded rendering) does not need them.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};
use serde::Deserialize;

use crate::paths;

enum CheckOutcome {
    Pass(String),
    Warn(String),
    Fail(String),
}

fn print_check(label: &str, outcome: &CheckOutcome) {
    let (symbol, detail) = match outcome {
        CheckOutcome::Pass(detail) => ("✓", detail),
        CheckOutcome::Warn(detail) => ("!", detail),
        CheckOutcome::Fail(detail) => ("✗", detail),
    };
    println!("{symbol} {label:<18} {detail}");
}

fn first_output_line(program: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    // `python3 --version` historically printed to stderr; accept either.
    let text = if output.stdout.is_empty() {
        output.stderr
    } else {
        output.stdout
    };
    String::from_utf8_lossy(&text)
        .lines()
        .next()
        .map(str::to_owned)
}

fn check_ffmpeg() -> CheckOutcome {
    let Some(ffmpeg) = paths::ffmpeg_executable() else {
        return CheckOutcome::Fail("not found (brew install ffmpeg, or set TONO_FFMPEG)".into());
    };
    let Some(version_line) = first_output_line(&ffmpeg, &["-hide_banner", "-version"]) else {
        return CheckOutcome::Fail(format!("{} is not runnable", ffmpeg.display()));
    };
    let encoders = Command::new(&ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default();
    // Output targets are H.264 + AAC (spec "MP4 rendering").
    let missing: Vec<&str> = [" libx264 ", " aac "]
        .into_iter()
        .filter(|encoder| !encoders.contains(encoder))
        .map(str::trim)
        .collect();
    if !missing.is_empty() {
        return CheckOutcome::Fail(format!("missing encoders: {}", missing.join(", ")));
    }
    CheckOutcome::Pass(format!("{version_line} ({})", ffmpeg.display()))
}

fn check_ffprobe() -> CheckOutcome {
    let Some(ffprobe) = paths::ffprobe_executable() else {
        return CheckOutcome::Fail("not found (ships with ffmpeg, or set TONO_FFPROBE)".into());
    };
    // Must actually run: a present-but-broken binary would let MP4
    // validation fail only at the very end of a long render.
    match first_output_line(&ffprobe, &["-hide_banner", "-version"]) {
        Some(version_line) if version_line.starts_with("ffprobe version") => {
            CheckOutcome::Pass(format!("{version_line} ({})", ffprobe.display()))
        }
        Some(unexpected) => {
            CheckOutcome::Fail(format!("{} printed `{unexpected}`", ffprobe.display()))
        }
        None => CheckOutcome::Fail(format!("{} is not runnable", ffprobe.display())),
    }
}

fn check_python() -> CheckOutcome {
    let Some(python) = paths::python_executable() else {
        return CheckOutcome::Fail("python3 not found (set TONO_PYTHON)".into());
    };
    match first_output_line(&python, &["--version"]) {
        Some(version) => CheckOutcome::Pass(format!("{version} ({})", python.display())),
        None => CheckOutcome::Fail(format!("{} is not runnable", python.display())),
    }
}

fn check_virtualenv(require_ml: bool) -> CheckOutcome {
    let venv = paths::ml_virtualenv_dir();
    if paths::ml_environment_python().is_file() {
        CheckOutcome::Pass(venv.display().to_string())
    } else if require_ml {
        CheckOutcome::Fail(format!("{} missing", venv.display()))
    } else {
        CheckOutcome::Warn(format!("{} missing (needed from M2)", venv.display()))
    }
}

/// Mirrors the JSON printed by `ml/analyze.py --self-check`.
#[derive(Deserialize)]
struct WorkerSelfCheck {
    version: u32,
    python: String,
    /// Real import attempts, keyed by module name.
    modules: BTreeMap<String, ComponentStatus>,
    /// Model weights present locally, keyed by description.
    models: BTreeMap<String, ComponentStatus>,
}

#[derive(Deserialize)]
struct ComponentStatus {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    detail: Option<String>,
}

/// ML components are warnings by default (M0/M1 do not need them) and
/// failures with `--ml`, which later milestones should run.
fn component_outcome(status: ComponentStatus, require_ml: bool) -> CheckOutcome {
    let message = status.error.or(status.detail).unwrap_or_default();
    match (status.ok, require_ml) {
        (true, _) => CheckOutcome::Pass(if message.is_empty() {
            "ok".into()
        } else {
            message
        }),
        (false, true) => CheckOutcome::Fail(message),
        (false, false) => CheckOutcome::Warn(format!("{message} (needed from M2)")),
    }
}

/// Runs the worker's self-check. Returns the worker outcome plus one
/// outcome per ML module/model it reported.
fn check_ml_worker(require_ml: bool) -> (CheckOutcome, Vec<(String, CheckOutcome)>) {
    let Ok(mut command) = paths::python_command() else {
        return (
            CheckOutcome::Fail("no python; run tono setup".into()),
            Vec::new(),
        );
    };
    let script = paths::ml_worker_script();
    let output = match command.arg(&script).arg("--self-check").output() {
        Ok(output) => output,
        Err(error) => {
            return (
                CheckOutcome::Fail(format!("spawn failed: {error}")),
                Vec::new(),
            )
        }
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return (
            CheckOutcome::Fail(format!(
                "{} --self-check failed: {}",
                script.display(),
                stderr.trim()
            )),
            Vec::new(),
        );
    }
    let report: WorkerSelfCheck = match serde_json::from_slice(&output.stdout) {
        Ok(report) => report,
        Err(error) => {
            return (
                CheckOutcome::Fail(format!("self-check JSON invalid: {error}")),
                Vec::new(),
            )
        }
    };
    let component_outcomes = report
        .modules
        .into_iter()
        .map(|(module, status)| {
            (
                format!("import {module}"),
                component_outcome(status, require_ml),
            )
        })
        .chain(
            report
                .models
                .into_iter()
                .map(|(model, status)| (model, component_outcome(status, require_ml))),
        )
        .collect();
    (
        CheckOutcome::Pass(format!(
            "contract v{} on python {}",
            report.version, report.python
        )),
        component_outcomes,
    )
}

pub fn run_doctor(require_ml: bool) -> Result<()> {
    println!("Tono doctor\n");
    let mut hard_failures = 0;
    let mut record = |label: &str, outcome: CheckOutcome| {
        if matches!(outcome, CheckOutcome::Fail(_)) {
            hard_failures += 1;
        }
        print_check(label, &outcome);
    };

    record("ffmpeg", check_ffmpeg());
    record("ffprobe", check_ffprobe());
    record("python", check_python());
    record("ml virtualenv", check_virtualenv(require_ml));
    let (worker_outcome, module_outcomes) = check_ml_worker(require_ml);
    record("ml worker", worker_outcome);
    for (module, outcome) in module_outcomes {
        record(&format!("  ml: {module}"), outcome);
    }

    println!();
    if hard_failures > 0 {
        bail!("{hard_failures} required check(s) failed");
    }
    if require_ml {
        println!("✓ environment ready including ML models");
    } else {
        println!("✓ base environment ready ('!' items are needed from M2; check with `tono doctor --ml`)");
    }
    Ok(())
}
