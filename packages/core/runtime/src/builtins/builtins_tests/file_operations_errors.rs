use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::RuntimeError;
use crate::builtins::*;

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|error| panic!("system clock before unix epoch: {error}"))
        .as_nanos()
}

fn scratch_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ncl-file-errors-{label}-{}", nonce()))
}

#[test]
fn probe_file_reports_non_not_found_io_errors() {
    let path = Value::string("/tmp/ncl-probe-\0-invalid");
    let result = probe_file(&[path]);
    assert!(matches!(
        result,
        Err(RuntimeError::Io { kind, .. }) if kind == std::io::ErrorKind::InvalidInput
    ));
}

#[test]
fn rename_file_rejects_non_string_new_path() {
    assert!(rename_file(&[Value::string("anything"), Value::Integer(2)]).is_err());
}

#[test]
fn rename_file_reports_canonicalize_error_for_missing_source() {
    let source = scratch_path("missing-source");
    let target = scratch_path("missing-target");
    let result = rename_file(&[
        Value::string(source.to_string_lossy().to_string()),
        Value::string(target.to_string_lossy().to_string()),
    ]);
    assert!(matches!(result, Err(RuntimeError::Io { .. })));
}

#[test]
fn rename_file_reports_error_when_target_parent_is_missing() {
    let source = scratch_path("rename-source");
    fs::write(&source, "content").unwrap_or_else(|error| panic!("write source file: {error}"));
    let target = std::env::temp_dir()
        .join(format!("ncl-missing-parent-{}", nonce()))
        .join("target.txt");
    let result = rename_file(&[
        Value::string(source.to_string_lossy().to_string()),
        Value::string(target.to_string_lossy().to_string()),
    ]);
    assert!(matches!(result, Err(RuntimeError::Io { .. })));
    let _ = fs::remove_file(&source);
}

#[test]
fn file_write_date_reports_error_for_missing_path() {
    let path = scratch_path("write-date-missing");
    let result = file_write_date(&[Value::string(path.to_string_lossy().to_string())]);
    assert!(matches!(result, Err(RuntimeError::Io { .. })));
}

#[test]
fn file_write_date_reports_error_for_pre_epoch_modification_time() {
    let path = scratch_path("write-date-pre-epoch");
    fs::write(&path, "content").unwrap_or_else(|error| panic!("write file: {error}"));
    let file = fs::File::options()
        .write(true)
        .open(&path)
        .unwrap_or_else(|error| panic!("open file for timestamp update: {error}"));
    let before_epoch = SystemTime::UNIX_EPOCH - Duration::from_hours(24);
    let times = fs::FileTimes::new().set_modified(before_epoch);
    file.set_times(times)
        .unwrap_or_else(|error| panic!("set pre-epoch modification time: {error}"));
    drop(file);
    let result = file_write_date(&[Value::string(path.to_string_lossy().to_string())]);
    assert!(matches!(result, Err(RuntimeError::Io { .. })));
    let _ = fs::remove_file(&path);
}

#[test]
fn truename_reports_error_for_missing_path() {
    let path = scratch_path("truename-missing");
    let result = truename(&[Value::string(path.to_string_lossy().to_string())]);
    assert!(matches!(result, Err(RuntimeError::Io { .. })));
}

#[test]
fn open_input_file_reports_write_error_when_parent_directory_is_missing() {
    let path = std::env::temp_dir()
        .join(format!("ncl-missing-parent-{}", nonce()))
        .join("input.txt");
    assert!(open_input_file(&path, "CREATE", false).is_err());
}

#[test]
fn open_input_file_reports_read_error_for_directory_path() {
    let directory = std::env::temp_dir();
    assert!(open_input_file(&directory, "ERROR", false).is_err());
}

#[test]
fn open_output_file_reports_append_read_error_for_directory_path() {
    let directory = std::env::temp_dir();
    assert!(open_output_file(&directory, "CREATE", "APPEND", false).is_err());
}

#[test]
fn open_io_file_reports_read_errors_for_directory_path() {
    let directory = std::env::temp_dir();
    assert!(open_io_file(&directory, "CREATE", "APPEND", false).is_err());
    assert!(open_io_file(&directory, "CREATE", "OVERWRITE", false).is_err());
}

#[test]
fn open_io_file_covers_missing_path_if_does_not_exist_arms() {
    let create_path = scratch_path("io-missing-create");
    let created = open_io_file(&create_path, "CREATE", "APPEND", false)
        .unwrap_or_else(|error| panic!("expected create to succeed: {error}"));
    assert!(matches!(created, Value::Stream(_)));

    let nil_path = scratch_path("io-missing-nil");
    let nil_result = open_io_file(&nil_path, "NIL", "APPEND", false)
        .unwrap_or_else(|error| panic!("expected nil to succeed: {error}"));
    assert!(matches!(nil_result, Value::Nil));

    let _ = fs::remove_file(&create_path);
}
