#[cfg(test)]
mod tests {
    use crate::Stream;

    #[test]
    fn input_streams_cover_cursor_and_line_boundaries_from_table_cases() {
        let cases = [
            ("string", Stream::input("ab\ncd", 0, 5)),
            ("file", Stream::file_input("ab\ncd")),
        ];
        for (name, mut stream) in cases {
            assert_eq!(
                stream.kind_name(),
                if name == "file" {
                    "FILE-INPUT-STREAM"
                } else {
                    "STRING-INPUT-STREAM"
                }
            );
            assert!(stream.is_input());
            assert!(!stream.is_output());
            assert_eq!(stream.peek_char(), Some('a'));
            assert_eq!(stream.read_char(), Some('a'));
            assert!(!stream.unread_char('x'));
            assert!(stream.unread_char('a'));
            assert!(!stream.unread_char('a'));
            assert_eq!(stream.read_line(), Some(("ab".into(), false)));
            assert_eq!(stream.remaining_input(), Some("cd".into()));
            assert!(stream.consume_input(1));
            assert_eq!(stream.remaining_input(), Some("d".into()));
            assert!(!stream.consume_input(2));
            assert!(stream.consume_input(0));
            assert_eq!(stream.read_line(), Some(("d".into(), true)));
            assert_eq!(stream.read_line(), None);
        }
    }
}
