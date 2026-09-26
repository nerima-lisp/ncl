use super::*;

#[test]
fn weak_key_entry_is_removed_when_key_dies() {
    weak_entry_after_gc(Weakness::Key, false, false, 0, Word::NIL);
}

#[test]
fn weak_value_entry_is_removed_when_value_dies() {
    weak_entry_after_gc(Weakness::Value, true, false, 0, Word::NIL);
}

#[test]
fn weak_key_and_value_entry_requires_both_referents() {
    weak_entry_after_gc(Weakness::KeyAndValue, false, false, 0, Word::NIL);
}

#[test]
fn weak_key_or_value_entry_survives_when_key_is_live() {
    weak_entry_after_gc(Weakness::KeyOrValue, true, false, 1, Word::fixnum(99));
}
