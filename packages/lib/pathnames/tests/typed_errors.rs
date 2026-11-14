//! Mapping tests for pathname errors.

use ncl_lib_pathnames::PathnameError;
use ncl_object::typed::ConditionErrorData;
use ncl_object::{FileError as LispFileError, LispError, Word};

fn assert_pathname_mapping(error: PathnameError, expected: LispFileError, display: &str) {
    assert_eq!(error.to_string(), display);

    match LispError::from(error) {
        LispError::Condition(ConditionErrorData::FileError { kind, pathname }) => {
            assert_eq!(kind, expected);
            assert_eq!(pathname, Word::fixnum(47));
        }
        other => panic!("unexpected pathname error mapping: {other:?}"),
    }
}

#[test]
fn pathname_errors_preserve_the_path_and_category() {
    assert_pathname_mapping(
        PathnameError::NotFound {
            pathname: Word::fixnum(47),
        },
        LispFileError::NotFound,
        "file was not found",
    );
    assert_pathname_mapping(
        PathnameError::PermissionDenied {
            pathname: Word::fixnum(47),
        },
        LispFileError::PermissionDenied,
        "file access was denied",
    );
    assert_pathname_mapping(
        PathnameError::InvalidPath {
            pathname: Word::fixnum(47),
        },
        LispFileError::InvalidPath,
        "invalid pathname",
    );
}
