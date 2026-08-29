#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::environment::Environment;
    use crate::value::{ClassDefinition, Value};

    #[test]
    fn comparisons_cover_runtime_identity_and_structural_variants() {
        let condition = Value::condition_from_parts_with_types(
            "ERROR".to_owned(),
            vec!["ERROR".to_owned()],
            vec![("DETAIL".to_owned(), Value::Integer(1))],
            "failed".to_owned(),
            None,
            Vec::new(),
        );
        let equivalent_condition = Value::condition_from_parts_with_types(
            "ERROR".to_owned(),
            vec!["ERROR".to_owned()],
            vec![("DETAIL".to_owned(), Value::Integer(1))],
            "failed".to_owned(),
            None,
            Vec::new(),
        );
        let structure = Value::structure_with_types(
            "POINT",
            vec![("X".to_owned(), Value::Integer(1))],
            Vec::new(),
        );
        let equivalent_structure = Value::structure_with_types(
            "POINT",
            vec![("X".to_owned(), Value::Integer(1))],
            Vec::new(),
        );
        let class = Rc::new(ClassDefinition {
            name: "POINT".to_owned(),
            precedence: vec!["POINT".to_owned()],
            slots: Vec::new(),
            default_initargs: Vec::new(),
        });
        let same_class = Value::class_object(Rc::clone(&class));
        let equivalent_class = Value::class_object(Rc::new(ClassDefinition {
            name: "point".to_owned(),
            precedence: vec!["point".to_owned()],
            slots: Vec::new(),
            default_initargs: Vec::new(),
        }));
        let instance =
            Value::instance(Rc::clone(&class), vec![("X".to_owned(), Value::Integer(1))]);
        let equivalent_instance =
            Value::instance(Rc::clone(&class), vec![("x".to_owned(), Value::Integer(1))]);
        let environment = Value::environment(Environment::new());
        let equivalent_environment = Value::environment(Environment::new());
        let cases = [
            (condition.clone(), condition.clone(), true, true),
            (condition.clone(), equivalent_condition.clone(), false, true),
            (structure.clone(), structure.clone(), true, true),
            (structure.clone(), equivalent_structure.clone(), false, true),
            (same_class.clone(), same_class.clone(), true, true),
            (same_class, equivalent_class, false, true),
            (instance.clone(), instance.clone(), true, true),
            (instance.clone(), equivalent_instance.clone(), false, true),
            (environment.clone(), environment.clone(), true, true),
            (environment, equivalent_environment, false, false),
            (
                Value::restart("retry"),
                Value::restart("retry"),
                false,
                false,
            ),
            (
                Value::builtin("A", |_| Ok(Value::Nil)),
                Value::builtin("A", |_| Ok(Value::Nil)),
                false,
                false,
            ),
        ];

        for (index, (left, right, eq_expected, equal_expected)) in cases.into_iter().enumerate() {
            assert_eq!(left.eq_value(&right), eq_expected, "case {index}");
            assert_eq!(left.equal_value(&right), equal_expected, "case {index}");
        }
        assert!(condition.equal_value(&equivalent_condition));
        assert!(structure.equal_value(&equivalent_structure));
        assert!(instance.equal_value(&equivalent_instance));
    }
}
