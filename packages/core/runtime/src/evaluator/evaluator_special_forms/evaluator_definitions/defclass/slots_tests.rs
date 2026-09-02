#[cfg(test)]
mod tests {
    use crate::{Runtime, RuntimeError};

    fn eval_err(source: &str) -> RuntimeError {
        match Runtime::new().eval_source(source) {
            Ok(values) => panic!("expected an error, got {values:?}"),
            Err(error) => error,
        }
    }

    #[test]
    fn defclass_accepts_a_bare_symbol_slot() {
        let values = Runtime::new()
            .eval_source("(defclass bare-slot-class () (a))")
            .unwrap_or_else(|error| {
                panic!("a slot given as a bare symbol should be accepted: {error}")
            });
        assert_eq!(values[0].to_string(), "BARE-SLOT-CLASS");
    }

    #[test]
    fn defclass_rejects_an_empty_list_slot() {
        let error = eval_err("(defclass empty-slot-class () (()))");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defclass slot must be a symbol or non-empty list"
        ));
    }

    #[test]
    fn defclass_rejects_an_invalid_slot_allocation() {
        let error = eval_err("(defclass invalid-allocation-class () ((value :allocation :local)))");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defclass allocation must be :instance or :class"
        ));
    }

    #[test]
    fn defclass_accepts_explicit_instance_allocation() {
        let values = Runtime::new()
            .eval_source("(defclass explicit-instance-class () ((value :allocation :instance)))")
            .unwrap_or_else(|error| {
                panic!("explicit instance allocation should be accepted: {error}")
            });
        assert_eq!(values[0].to_string(), "EXPLICIT-INSTANCE-CLASS");
    }

    #[test]
    fn defclass_accepts_multiple_initargs_for_one_slot() {
        let values = Runtime::new()
            .eval_source(
                "(defclass aliases-class () ((value :initarg :value :initarg :alternate)))
                 (slot-value (make-instance 'aliases-class :alternate 42) 'value)",
            )
            .unwrap_or_else(|error| panic!("a slot should accept each declared initarg: {error}"));
        assert_eq!(values[1].to_string(), "42");
    }

    #[test]
    fn defclass_rejects_non_string_slot_documentation() {
        let error =
            eval_err("(defclass invalid-slot-documentation () ((value :documentation 42)))");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defclass slot documentation must be a string"
        ));
    }

    #[test]
    fn slot_definition_accessors_return_declared_metadata() {
        let values = Runtime::new()
            .eval_source(
                "(defclass documented-slot ()
                   ((value :documentation \"slot doc\" :initarg :value
                           :initform 42 :type integer
                           :reader value-reader :writer value-writer)
                    (class-value :allocation :class)))
                 (list
                   (slot-definition-documentation
                     (first (class-direct-slots (find-class 'documented-slot))))
                   (slot-definition-initargs
                     (first (class-direct-slots (find-class 'documented-slot))))
                   (slot-definition-allocation
                     (second (class-direct-slots (find-class 'documented-slot))))
                   (slot-definition-initform
                     (first (class-direct-slots (find-class 'documented-slot))))
                   (functionp
                     (slot-definition-initfunction
                       (first (class-direct-slots (find-class 'documented-slot)))))
                   (slot-definition-type
                     (first (class-direct-slots (find-class 'documented-slot))))
                   (slot-definition-readers
                     (first (class-direct-slots (find-class 'documented-slot))))
                   (slot-definition-writers
                     (first (class-direct-slots (find-class 'documented-slot)))))",
            )
            .unwrap();
        assert_eq!(
            values.last().unwrap().to_string(),
            "(\"slot doc\" (:VALUE) :CLASS 42 T INTEGER (VALUE-READER) (VALUE-WRITER))"
        );
    }

    #[test]
    fn defclass_accessor_is_setfable() {
        let values = Runtime::new()
            .eval_source(
                "(defclass accessor-class () ((value :accessor accessor-value)))
                 (let ((object (make-instance 'accessor-class)))
                   (setf (accessor-value object) 42)
                   (accessor-value object))",
            )
            .unwrap();
        assert_eq!(values.last().unwrap().to_string(), "42");
    }
}
