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
    fn using_class_slot_primitives_operate_on_an_instance() {
        let class = empty_class("POINT");
        let instance =
            Value::instance(class.clone(), vec![("value".to_owned(), Value::Integer(7))]);
        let class_value = Value::Class(class);
        let value = Runtime::apply_slot_primitive(
            "SLOT-VALUE-USING-CLASS",
            &[
                class_value.clone(),
                instance.clone(),
                Value::symbol("value"),
            ],
            SPAN,
        )
        .expect("using-class primitive is recognized")
        .expect("slot value succeeds");
        assert!(matches!(value, Value::Integer(7)));
        let exists = Runtime::apply_slot_primitive(
            "SLOT-EXISTS-P-USING-CLASS",
            &[
                class_value.clone(),
                instance.clone(),
                Value::symbol("value"),
            ],
            SPAN,
        )
        .expect("using-class exists primitive is recognized")
        .expect("slot exists succeeds");
        assert!(matches!(exists, Value::Boolean(true)));
        let bound = Runtime::apply_slot_primitive(
            "SLOT-BOUNDP-USING-CLASS",
            &[
                class_value.clone(),
                instance.clone(),
                Value::symbol("value"),
            ],
            SPAN,
        )
        .expect("using-class primitive is recognized")
        .expect("slot boundp succeeds");
        assert!(matches!(bound, Value::Boolean(true)));
        Runtime::apply_slot_primitive(
            "SLOT-MAKUNBOUND-USING-CLASS",
            &[class_value, instance.clone(), Value::symbol("value")],
            SPAN,
        )
        .expect("using-class primitive is recognized")
        .expect("slot makunbound succeeds");
        assert!(!instance.instance_slot_is_bound("value").unwrap_or(false));
    }

    #[test]
    fn compiled_setf_using_class_slot_updates_the_instance() {
        let values = Runtime::new()
            .eval_compiled_source(
                "(defclass point () ((value :initarg :value)))
                 (let ((point (make-instance 'point :value 1)))
                   (setf (slot-value-using-class (find-class 'point) point 'value) 9)
                   (slot-value point 'value))",
            )
            .expect("compiled using-class SETF succeeds");
        assert!(matches!(values.last(), Some(Value::Integer(9))));
    }

    #[test]
    fn compiled_slot_exists_p_using_class_accepts_three_arguments() {
        let values = Runtime::new()
            .eval_compiled_source(
                "(defclass exists-point () ((value)))
                 (let ((point (make-instance 'exists-point)))
                   (slot-exists-p-using-class (find-class 'exists-point) point 'value))",
            )
            .expect("compiled using-class SLOT-EXISTS-P succeeds");
        assert!(matches!(values.last(), Some(Value::Boolean(true))));
    }

    #[test]
    fn slot_value_dispatches_missing_slots_to_slot_missing() {
        let values = Runtime::new()
            .eval_source(
                "(defclass missing-slot-object () ())
                 (defmethod slot-missing ((class t) (object missing-slot-object)
                                           (slot-name t) (operation t))
                   (declare (ignore class object slot-name operation))
                   42)
                 (slot-value (make-instance 'missing-slot-object) 'absent)",
            )
            .expect("SLOT-MISSING method handles an undefined slot");
        assert!(matches!(values.last(), Some(Value::Integer(42))));
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
                init_function: None,
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
                    init_function: None,
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
                    init_function: None,
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
                    init_function: None,
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
                    init_function: None,
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

    #[test]
    fn change_class_replaces_class_and_preserves_shared_slot_names() {
        let runtime = Runtime::new();
        let values = runtime
            .eval_compiled_source(
                "(defclass old-point () ((x)))
                 (defclass new-point () ((x) (y)))
                 (let ((point (make-instance 'old-point)))
                   (setf (slot-value point 'x) 7)
                   (change-class point 'new-point)
                   (list (class-name (class-of point))
                         (slot-value point 'x)
                         (slot-boundp point 'y)))",
            )
            .expect("compiled change-class succeeds");
        let items = values.last().unwrap().list_items().unwrap();
        assert_eq!(items[0].to_string(), "NEW-POINT");
        assert!(matches!(items[1], Value::Integer(7)));
        assert!(matches!(items[2], Value::Nil | Value::Boolean(false)));
    }

    #[test]
    fn change_class_dispatches_update_instance_for_different_class() {
        let runtime = Runtime::new();
        let values = runtime
            .eval_compiled_source(
                "(defclass old-point () ((x)))
                 (defclass new-point () ((x) (updated)))
                 (defmethod update-instance-for-different-class
                   ((old old-point) (new new-point))
                   (setf (slot-value new 'updated) t)
                   new)
                 (let ((point (make-instance 'old-point)))
                   (change-class point 'new-point)
                   (slot-value point 'updated))",
            )
            .expect("compiled change-class hook succeeds");
        assert!(matches!(values.last().unwrap(), Value::Boolean(true)));
    }
}
