#![allow(missing_docs)]

use std::process::{Command, Output, Stdio};

fn ncl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ncl"))
}

fn output(command: &mut Command) -> Output {
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap_or_else(|error| panic!("failed to run ncl: {error}"))
}

#[test]
fn optimization_modes_preserve_stdout_and_gate_diagnostics() {
    for mode in ["default", "off"] {
        let option = format!("--optimization={mode}");
        let result = output(ncl().args([option.as_str(), "--eval", "42"]));
        assert!(result.status.success(), "{result:?}");
        assert_eq!(String::from_utf8_lossy(&result.stdout), "42\n");
        assert!(result.stderr.is_empty());
    }

    let diagnostic = output(ncl().args(["--opt-diagnostics", "--eval", "42"]));
    assert!(diagnostic.status.success(), "{diagnostic:?}");
    assert_eq!(String::from_utf8_lossy(&diagnostic.stdout), "42\n");
    let stderr = String::from_utf8_lossy(&diagnostic.stderr);
    assert!(stderr.contains("pass=\"sccp\""));
    assert!(stderr.contains("pass=\"dead-code-elimination\""));
    assert!(stderr.contains("function=\""));
    assert!(stderr.contains("iteration=1"));
    assert!(stderr.contains("changed="));
    assert!(stderr.contains("verification=passed"));
    assert!(stderr.contains("elapsed_ns="));

    let disabled = output(ncl().args(["--optimization=off", "--opt-diagnostics", "--eval", "42"]));
    assert!(disabled.status.success(), "{disabled:?}");
    assert_eq!(String::from_utf8_lossy(&disabled.stdout), "42\n");
    assert!(disabled.stderr.is_empty());
}

#[test]
fn optimization_options_apply_to_file_modes() {
    let path = std::env::temp_dir().join(format!(
        "ncl-optimization-options-{}.lisp",
        std::process::id()
    ));
    std::fs::write(&path, "43")
        .unwrap_or_else(|error| panic!("source file creation failed: {error}"));
    let path = path.to_string_lossy().into_owned();
    let fasl = std::path::Path::new(&path)
        .with_extension("fasl")
        .to_string_lossy()
        .into_owned();

    for mode in ["default", "off"] {
        let option = format!("--optimization={mode}");
        let loaded = output(ncl().args([option.as_str(), "--load", &path]));
        assert!(loaded.status.success(), "{loaded:?}");
        assert_eq!(String::from_utf8_lossy(&loaded.stdout), "43\n");

        let compiled = output(ncl().args([option.as_str(), "--compile-file", &path]));
        assert!(compiled.status.success(), "{compiled:?}");
        assert_eq!(String::from_utf8_lossy(&compiled.stdout), "43\n");

        let reloaded = output(ncl().args([option.as_str(), "--load", &fasl]));
        assert!(reloaded.status.success(), "{reloaded:?}");
        assert_eq!(String::from_utf8_lossy(&reloaded.stdout), "43\n");

        let script = output(ncl().args([option.as_str(), "--script", &path]));
        assert!(script.status.success(), "{script:?}");
        assert!(script.stdout.is_empty());
    }

    std::fs::remove_file(&path).unwrap_or_else(|error| panic!("source cleanup failed: {error}"));
    std::fs::remove_file(&fasl).unwrap_or_else(|error| panic!("FASL cleanup failed: {error}"));
}

#[test]
fn optimization_mode_rejects_unknown_values() {
    let result = output(ncl().args(["--optimization=unsafe", "--eval", "42"]));
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("unsupported optimization mode"));
}
