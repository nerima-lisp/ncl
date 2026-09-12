use super::super::super::Value;

struct FailingWriter {
    budget: usize,
}

impl std::fmt::Write for FailingWriter {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        if text.len() > self.budget {
            self.budget = 0;
            return Err(std::fmt::Error);
        }
        self.budget -= text.len();
        Ok(())
    }
}

#[test]
fn display_propagates_downstream_write_failures() {
    use std::fmt::Write as _;

    let structure = Value::structure_with_types(
        "point",
        vec![("x".to_owned(), Value::Integer(1))],
        Vec::new(),
    );
    let cases = [
        (Value::keyword_exact("k"), 0),
        (Value::list(vec![Value::Integer(1)]), 0),
        (Value::list(vec![Value::Integer(1)]), 1),
        (Value::list(vec![Value::Integer(1), Value::Integer(2)]), 2),
        (
            Value::dotted_list(vec![Value::Integer(1)], Value::Integer(2)),
            0,
        ),
        (
            Value::dotted_list(vec![Value::Integer(1)], Value::Integer(2)),
            1,
        ),
        (
            Value::dotted_list(vec![Value::Integer(1)], Value::Integer(2)),
            2,
        ),
        (Value::vector(vec![Value::Integer(1)]), 0),
        (Value::vector(vec![Value::Integer(1)]), 2),
        (Value::values(vec![Value::Integer(1)]), 0),
        (Value::values(vec![Value::Integer(1)]), 8),
        (Value::values(vec![Value::Integer(1)]), 9),
        (structure.clone(), 0),
        (structure, "#S(point".len()),
        (Value::symbol_exact("A"), 0),
        (Value::symbol_exact("|"), 1),
        (Value::symbol_exact("A"), 1),
    ];

    for (value, budget) in cases {
        let mut writer = FailingWriter { budget };
        assert!(write!(writer, "{value}").is_err());
    }
}

#[test]
fn debug_propagates_downstream_write_failures() {
    use std::fmt::Write as _;

    for budget in [0, "Value(".len()] {
        let mut writer = FailingWriter { budget };
        assert!(write!(writer, "{:?}", Value::Nil).is_err());
    }
}
