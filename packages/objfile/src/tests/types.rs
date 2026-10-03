use super::*;

#[test]
fn target_usize_preserves_representable_values() {
    assert_eq!(target_usize(17, "offset", 99), Ok(17));
}
