use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

fn path_value(path: &Path) -> Value {
    Value::string(path.to_string_lossy().to_string())
}

fn temporary_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|error| panic!("system clock error: {error}"))
        .as_nanos();
    std::env::temp_dir().join(format!("ncl-{label}-{}-{nonce}", std::process::id()))
}

#[test]
fn file_operations_cover_success_and_failure_boundaries() {
    let source = temporary_path("file-operations-source");
    let renamed = temporary_path("file-operations-renamed");
    if let Err(error) = fs::write(&source, "content") {
        panic!("failed to create test file: {error}");
    }

    assert!(matches!(
        probe_file(&[path_value(&source)]),
        Ok(Value::String(_))
    ));
    assert!(matches!(
        probe_file(&[path_value(&renamed)]),
        Ok(Value::Nil)
    ));
    assert!(file_write_date(&[path_value(&source)]).is_ok());

    let renamed_result = rename_file(&[path_value(&source), path_value(&renamed)])
        .unwrap_or_else(|error| panic!("rename failed: {error}"));
    assert!(matches!(renamed_result, Value::Values(_)));
    assert!(matches!(
        delete_file(&[path_value(&renamed)]),
        Ok(Value::Boolean(true))
    ));
    assert!(delete_file(&[path_value(&renamed)]).is_err());
}

#[test]
fn output_file_options_are_table_driven() {
    let existing = temporary_path("open-output-existing");
    let missing = temporary_path("open-output-missing");
    if let Err(error) = fs::write(&existing, "old") {
        panic!("failed to create test file: {error}");
    }

    let existing_cases = [
        ("NIL", true),
        ("ERROR", false),
        ("APPEND", true),
        ("NEW-VERSION", true),
        ("RENAME", true),
        ("RENAME-AND-DELETE", true),
        ("OVERWRITE", true),
        ("SUPERSEDE", true),
    ];
    for (option, succeeds) in existing_cases {
        let result = open_output_file(&existing, "CREATE", option);
        assert_eq!(result.is_ok(), succeeds, "if-exists={option}");
    }
    for (index, (option, succeeds)) in [("CREATE", true), ("NIL", true), ("ERROR", false)]
        .into_iter()
        .enumerate()
    {
        let path = missing.with_extension(format!("case-{index}"));
        let result = open_output_file(&path, option, "NEW-VERSION");
        assert_eq!(result.is_ok(), succeeds, "if-does-not-exist={option}");
        let _ = fs::remove_file(path);
    }
    assert!(open_output_file(&existing, "CREATE", "UNKNOWN").is_err());
    assert!(open_output_file(&missing, "UNKNOWN", "NEW-VERSION").is_err());

    let _ = fs::remove_file(existing);
    let _ = fs::remove_file(missing);
}

#[test]
fn input_and_io_file_options_are_table_driven() {
    let existing = temporary_path("open-input-existing");
    let missing = temporary_path("open-input-missing");
    assert!(fs::write(&existing, "content").is_ok());

    for (option, succeeds) in [("NIL", true), ("ERROR", true)] {
        let result = open_input_file(&existing, option);
        assert_eq!(result.is_ok(), succeeds, "existing input option={option}");
    }
    for (index, (option, succeeds)) in [("NIL", true), ("CREATE", true), ("ERROR", false)]
        .into_iter()
        .enumerate()
    {
        let path = missing.with_extension(format!("case-{index}"));
        let result = open_input_file(&path, option);
        assert_eq!(result.is_ok(), succeeds, "missing input option={option}");
        let _ = fs::remove_file(path);
    }
    assert!(open_input_file(&missing, "UNKNOWN").is_err());

    for (option, succeeds) in [
        ("NIL", true),
        ("ERROR", false),
        ("APPEND", true),
        ("NEW-VERSION", true),
        ("RENAME", true),
        ("RENAME-AND-DELETE", true),
        ("OVERWRITE", true),
        ("SUPERSEDE", true),
    ] {
        let result = open_io_file(&existing, "CREATE", option);
        assert_eq!(result.is_ok(), succeeds, "existing io option={option}");
    }
    assert!(open_io_file(&existing, "CREATE", "UNKNOWN").is_err());
    assert!(open_io_file(&missing, "UNKNOWN", "APPEND").is_err());

    let _ = fs::remove_file(existing);
    let _ = fs::remove_file(missing);
}
