//! Mapping tests for stream errors.

use ncl_lib_streams::StreamError;
use ncl_object::typed::ConditionErrorData;
use ncl_object::{LispError, StreamError as LispStreamError, Word};

fn assert_stream_mapping(error: StreamError, expected: LispStreamError, display: &str) {
    assert_eq!(error.to_string(), display);

    match LispError::from(error) {
        LispError::Condition(ConditionErrorData::StreamError { kind, stream }) => {
            assert_eq!(kind, expected);
            assert_eq!(stream, Word::fixnum(31));
        }
        other => panic!("unexpected stream error mapping: {other:?}"),
    }
}

#[test]
fn stream_errors_preserve_the_stream_and_category() {
    assert_stream_mapping(
        StreamError::Closed {
            stream: Word::fixnum(31),
        },
        LispStreamError::Closed,
        "stream is closed",
    );
    assert_stream_mapping(
        StreamError::InvalidDirection {
            stream: Word::fixnum(31),
        },
        LispStreamError::InvalidDirection,
        "invalid stream direction",
    );
    assert_stream_mapping(
        StreamError::Io {
            stream: Word::fixnum(31),
        },
        LispStreamError::Io,
        "stream input/output failed",
    );

    let end_of_file = StreamError::EndOfFile {
        stream: Word::fixnum(31),
    };
    assert_eq!(end_of_file.to_string(), "end of file");
    match LispError::from(end_of_file) {
        LispError::Condition(ConditionErrorData::EndOfFile { stream }) => {
            assert_eq!(stream, Word::fixnum(31));
        }
        other => panic!("unexpected end-of-file mapping: {other:?}"),
    }
}
