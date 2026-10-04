use super::{apply_case, capitalize, needs_escape, render_name};
use crate::PrintCase;

#[test]
fn rendering_covers_case_and_delimiter_rules() {
    assert_eq!(
        apply_case("Hello WORLD", PrintCase::Downcase),
        "hello world"
    );
    assert_eq!(capitalize("hello-WORLD 42"), "Hello-World 42");
    assert_eq!(
        render_name("a|b\\c", true, PrintCase::Upcase),
        "|a\\|b\\\\c|"
    );
    assert_eq!(render_name("hello", false, PrintCase::Upcase), "HELLO");
    assert!(needs_escape("", PrintCase::Upcase));
    assert!(needs_escape(".", PrintCase::Upcase));
    assert!(needs_escape("123", PrintCase::Upcase));
    assert!(needs_escape("a b", PrintCase::Upcase));
    assert!(!needs_escape("ABC", PrintCase::Upcase));
}
