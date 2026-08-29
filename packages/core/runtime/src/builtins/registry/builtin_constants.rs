use crate::Value;

pub(super) const fn constant_bindings() -> [(&'static str, Value); 4] {
    [
        ("NIL", Value::Nil),
        ("T", Value::boolean(true)),
        ("CHAR-CODE-LIMIT", Value::Integer(0x11_00_00)),
        ("MOST-POSITIVE-CHAR-CODE", Value::Integer(0x10_FF_FF)),
    ]
}
