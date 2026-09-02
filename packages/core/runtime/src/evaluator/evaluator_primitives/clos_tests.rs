#[cfg(test)]
mod tests {
    use super::super::*;

    const SPAN: Span = Span::new(0, 0);

    fn empty_class(name: &str) -> Rc<ClassDefinition> {
        Rc::new(ClassDefinition {
            name: name.to_string(),
            documentation: None,
            direct_superclasses: Vec::new(),
            direct_slots: Vec::new(),
            direct_default_initargs: Vec::new(),
            precedence: vec![
                name.to_string().into(),
                "STANDARD-OBJECT".to_string().into(),
            ],
            slots: Vec::new(),
            default_initargs: Vec::new(),
        })
    }

    #[test]
    fn slot_primitive_rejects_the_wrong_argument_count() {
        let result = Runtime::apply_slot_primitive("SLOT-VALUE", &[Value::Integer(1)], SPAN)
            .unwrap_or_else(|| panic!("SLOT-VALUE is a recognized slot primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::Arity { function, .. }) if function == "slot operation"
        ));
    }

    #[test]
    fn slot_primitive_rejects_a_non_instance_receiver() {
        let arguments = [Value::Integer(1), Value::symbol("name")];
        let result = Runtime::apply_slot_primitive("SLOT-VALUE", &arguments, SPAN)
            .unwrap_or_else(|| panic!("SLOT-VALUE is a recognized slot primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::Type { expected, actual, .. })
                if expected == "STANDARD-OBJECT" && actual == "INTEGER"
        ));
    }

    #[test]
    fn slot_makunbound_rejects_an_undefined_slot() {
        let instance = Value::instance(empty_class("POINT"), Vec::new());
        let arguments = [instance, Value::symbol("missing")];
        let result = Runtime::apply_slot_primitive("SLOT-MAKUNBOUND", &arguments, SPAN)
            .unwrap_or_else(|| panic!("SLOT-MAKUNBOUND is a recognized slot primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "slot is not defined for this class"
        ));
    }

    #[test]
    fn class_of_a_non_instance_value_synthesizes_a_class_definition() {
        let environment = Environment::new();
        let result = Runtime::apply_class_introspection_primitive(
            "CLASS-OF",
            &[Value::Integer(1)],
            &environment,
            SPAN,
        )
        .unwrap_or_else(|| panic!("CLASS-OF is a recognized introspection primitive"))
        .unwrap_or_else(|error| panic!("CLASS-OF on a non-instance value succeeds: {error}"));
        match result {
            Value::Class(definition) => assert_eq!(definition.name, "INTEGER"),
            other => panic!("expected a class value, got {other:?}"),
        }
    }

    #[test]
    fn class_name_rejects_a_non_class_argument() {
        let environment = Environment::new();
        let result = Runtime::apply_class_introspection_primitive(
            "CLASS-NAME",
            &[Value::Integer(1)],
            &environment,
            SPAN,
        )
        .unwrap_or_else(|| panic!("CLASS-NAME is a recognized introspection primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::Type { expected, .. }) if expected == "CLASS"
        ));
    }

    #[test]
    fn class_finalized_p_accepts_class_objects() {
        let environment = Environment::new();
        let result = Runtime::apply_class_introspection_primitive(
            "CLASS-FINALIZED-P",
            &[Value::class_object(empty_class("POINT"))],
            &environment,
            SPAN,
        )
        .unwrap()
        .unwrap();
        assert!(result.is_truthy());
    }

    #[test]
    fn class_finalized_p_rejects_non_class_objects() {
        let environment = Environment::new();
        let result = Runtime::apply_class_introspection_primitive(
            "CLASS-FINALIZED-P",
            &[Value::Integer(1)],
            &environment,
            SPAN,
        )
        .unwrap()
        .unwrap_err();
        assert!(matches!(result, RuntimeError::Type { expected, .. } if expected == "CLASS"));
    }

    #[test]
    fn class_direct_superclasses_returns_class_objects() {
        let environment = Environment::new();
        let class = Rc::new(ClassDefinition {
            name: "POINT".to_owned(),
            documentation: None,
            direct_superclasses: vec!["STANDARD-OBJECT".into()],
            direct_slots: vec!["X".into()],
            direct_default_initargs: Vec::new(),
            precedence: vec!["POINT".into(), "STANDARD-OBJECT".into()],
            slots: vec![ClassSlot {
                name: "X".to_owned(),
                documentation: None,
                initargs: Vec::new(),
                readers: Vec::new(),
                writers: Vec::new(),
                init_form: None,
                type_form: None,
                class_value: None,
            }],
            default_initargs: Vec::new(),
        });
        environment.define_class("POINT", Rc::clone(&class));
        let result = Runtime::apply_class_introspection_primitive(
            "CLASS-DIRECT-SUPERCLASSES",
            &[Value::class_object(class)],
            &environment,
            SPAN,
        )
        .unwrap_or_else(|| panic!("CLASS-DIRECT-SUPERCLASSES is recognized"))
        .unwrap_or_else(|error| panic!("class introspection succeeds: {error}"));
        assert!(matches!(result, Value::List(_)));
    }

    #[test]
    fn class_direct_slots_returns_slot_names() {
        let environment = Environment::new();
        let class = Rc::new(ClassDefinition {
            name: "POINT".to_owned(),
            documentation: None,
            direct_superclasses: vec!["STANDARD-OBJECT".into()],
            direct_slots: vec!["X".into(), "Y".into()],
            direct_default_initargs: Vec::new(),
            precedence: vec!["POINT".into(), "STANDARD-OBJECT".into()],
            slots: vec![
                ClassSlot {
                    name: "X".to_owned(),
                    documentation: None,
                    initargs: Vec::new(),
                    readers: Vec::new(),
                    writers: Vec::new(),
                    init_form: None,
                    type_form: None,
                    class_value: None,
                },
                ClassSlot {
                    name: "Y".to_owned(),
                    documentation: None,
                    initargs: Vec::new(),
                    readers: Vec::new(),
                    writers: Vec::new(),
                    init_form: None,
                    type_form: None,
                    class_value: None,
                },
            ],
            default_initargs: Vec::new(),
        });
        let result = Runtime::apply_class_introspection_primitive(
            "CLASS-DIRECT-SLOTS",
            &[Value::class_object(class)],
            &environment,
            SPAN,
        )
        .unwrap()
        .unwrap();
        let slots = result
            .list_items()
            .expect("class-direct-slots returns a list");
        assert_eq!(slots[0].instance_slot("NAME").unwrap().to_string(), "X");
        assert_eq!(slots[1].instance_slot("NAME").unwrap().to_string(), "Y");
    }

    #[test]
    fn class_slots_returns_effective_slot_names() {
        let environment = Environment::new();
        let class = Rc::new(ClassDefinition {
            name: "POINT".to_owned(),
            documentation: None,
            direct_superclasses: vec!["STANDARD-OBJECT".into()],
            direct_slots: vec!["X".into()],
            direct_default_initargs: Vec::new(),
            precedence: vec!["POINT".into(), "STANDARD-OBJECT".into()],
            slots: vec![
                ClassSlot {
                    name: "X".to_owned(),
                    documentation: None,
                    initargs: Vec::new(),
                    readers: Vec::new(),
                    writers: Vec::new(),
                    init_form: None,
                    type_form: None,
                    class_value: None,
                },
                ClassSlot {
                    name: "Y".to_owned(),
                    documentation: None,
                    initargs: Vec::new(),
                    readers: Vec::new(),
                    writers: Vec::new(),
                    init_form: None,
                    type_form: None,
                    class_value: None,
                },
            ],
            default_initargs: Vec::new(),
        });
        let result = Runtime::apply_class_introspection_primitive(
            "CLASS-SLOTS",
            &[Value::class_object(class)],
            &environment,
            SPAN,
        )
        .unwrap()
        .unwrap();
        let slots = result.list_items().expect("class-slots returns a list");
        assert_eq!(slots[0].instance_slot("NAME").unwrap().to_string(), "X");
        assert_eq!(slots[1].instance_slot("NAME").unwrap().to_string(), "Y");
    }
}
