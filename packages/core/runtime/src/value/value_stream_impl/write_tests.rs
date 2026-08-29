#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::Stream;

    #[test]
    fn output_streams_cover_buffer_and_newline_boundaries_from_table_cases() {
        let cases = [
            ("", true, ""),
            ("prefix", false, "prefix\n"),
            ("prefix\n", true, "prefix\n"),
        ];
        for (initial, at_line_start, expected) in cases {
            let mut stream = Stream::output();
            assert_eq!(stream.kind_name(), "STRING-OUTPUT-STREAM");
            assert!(!stream.is_input());
            assert!(stream.is_output());
            assert!(stream.write(initial));
            assert_eq!(stream.fresh_line(), Some(!at_line_start));
            assert_eq!(stream.take_output(), Some(expected.to_owned()));
            assert_eq!(stream.take_output(), Some(String::new()));
            assert!(stream.write("open"));
            if let Err(error) = stream.close(false) {
                panic!("closing string output stream: {error}");
            }
            assert!(!stream.write("closed"));
            assert_eq!(stream.fresh_line(), None);
        }
    }

    #[test]
    fn file_streams_cover_commit_and_abort_from_table_cases() {
        let base = std::env::temp_dir().join(format!("ncl-stream-{}", std::process::id()));
        let cases = [("output", "written", false), ("abort", "discarded", true)];
        for (suffix, contents, abort) in cases {
            let path = PathBuf::from(format!("{}-{suffix}", base.display()));
            let mut stream = Stream::file_output(path.clone(), String::new());
            assert!(stream.write(contents));
            if let Err(error) = stream.close(abort) {
                panic!("closing file output stream: {error}");
            }
            if abort {
                assert!(!path.exists());
            } else {
                let actual = match std::fs::read_to_string(&path) {
                    Ok(actual) => actual,
                    Err(error) => panic!("reading committed stream output: {error}"),
                };
                assert_eq!(actual, contents);
                if let Err(error) = std::fs::remove_file(&path) {
                    panic!("removing committed stream output: {error}");
                }
            }
        }
    }

    #[test]
    fn io_streams_cover_read_write_append_and_close_from_table_cases() {
        let base = std::env::temp_dir().join(format!("ncl-io-stream-{}", std::process::id()));
        let cases = [(false, "ab", "X", "aX"), (true, "ab", "X", "abX")];
        for (append, source, text, expected) in cases {
            let path = PathBuf::from(format!("{}-{append}", base.display()));
            let mut stream = Stream::file_io(path.clone(), source, append);
            assert_eq!(stream.kind_name(), "FILE-IO-STREAM");
            assert!(stream.is_input());
            assert!(stream.is_output());
            if append {
                assert_eq!(stream.read_char(), None);
            } else {
                assert_eq!(stream.read_char(), Some('a'));
            }
            assert!(stream.write(text));
            if let Err(error) = stream.close(false) {
                panic!("closing file IO stream: {error}");
            }
            let actual = match std::fs::read_to_string(&path) {
                Ok(actual) => actual,
                Err(error) => panic!("reading file IO stream output: {error}"),
            };
            assert_eq!(actual, expected);
            if let Err(error) = std::fs::remove_file(path) {
                panic!("removing file IO stream output: {error}");
            }
        }
    }
}
