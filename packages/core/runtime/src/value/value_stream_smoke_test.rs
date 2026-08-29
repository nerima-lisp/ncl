#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::super::Stream;

    #[test]
    fn streams_cover_input_pushback_output_and_close_state() {
        let mut input = Stream::input("ab\ncd", 0, 5);
        assert_eq!(input.kind_name(), "STRING-INPUT-STREAM");
        assert!(input.is_input());
        assert!(!input.is_output());
        assert_eq!(input.peek_char(), Some('a'));
        assert_eq!(input.read_char(), Some('a'));
        assert!(!input.unread_char('x'));
        assert!(input.unread_char('a'));
        assert!(!input.unread_char('a'));
        assert_eq!(input.read_line(), Some(("ab".to_owned(), false)));
        assert_eq!(input.remaining_input(), Some("cd".to_owned()));
        assert!(input.consume_input(2));
        assert_eq!(input.read_char(), None);
        assert!(!input.consume_input(1));
        if let Err(error) = input.close(true) {
            panic!("expected input close to succeed, got {error:?}");
        }
        assert!(input.peek_char().is_none());
        assert!(!input.unread_char('c'));
        assert!(input.close(false).is_ok());

        let mut output = Stream::output();
        assert_eq!(output.kind_name(), "STRING-OUTPUT-STREAM");
        assert!(!output.is_input());
        assert!(output.is_output());
        assert!(output.fresh_line().is_some());
        assert!(output.write("text"));
        assert_eq!(output.fresh_line(), Some(true));
        assert_eq!(output.take_output(), Some("text\n".to_owned()));
        assert_eq!(output.take_output(), Some(String::new()));
        assert!(output.read_char().is_none());
        if let Err(error) = output.close(true) {
            panic!("expected output close to succeed, got {error:?}");
        }
        assert!(!output.write("closed"));

        let mut io = Stream::file_io(PathBuf::from("unused"), "abc\n", true);
        assert_eq!(io.kind_name(), "FILE-IO-STREAM");
        assert!(io.is_input() && io.is_output());
        assert_eq!(io.peek_char(), None);
        assert_eq!(io.fresh_line(), Some(false));
        assert!(io.write("z"));
        if let Err(error) = io.close(true) {
            panic!("expected io close to succeed, got {error:?}");
        }
    }
}
