use std::any::Any;
use std::fmt;
use std::rc::Rc;

#[derive(Default)]
pub(super) struct EvaluatorAccessSlot(Option<Rc<dyn Any>>);

impl fmt::Debug for EvaluatorAccessSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(if self.0.is_some() {
            "EvaluatorAccessSlot(Some)"
        } else {
            "EvaluatorAccessSlot(None)"
        })
    }
}

impl super::ThreadContext {
    /// Replace the typed evaluator service association and return the previous association.
    #[must_use]
    pub fn replace_evaluator_access<T: Any>(
        &mut self,
        access: Option<Rc<T>>,
    ) -> Option<Rc<dyn Any>> {
        let access = access.map(|access| access as Rc<dyn Any>);
        std::mem::replace(&mut self.evaluator_access.0, access)
    }

    /// Restore a typed evaluator service association saved before nested evaluation.
    pub fn restore_evaluator_access(&mut self, access: Option<Rc<dyn Any>>) {
        self.evaluator_access.0 = access;
    }

    /// Return the active evaluator service when it has the requested concrete type.
    #[must_use]
    pub fn evaluator_access<T: Any>(&self) -> Option<Rc<T>> {
        self.evaluator_access
            .0
            .as_ref()?
            .clone()
            .downcast::<T>()
            .ok()
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::super::ThreadContext;
    use super::EvaluatorAccessSlot;

    #[test]
    fn evaluator_access_slot_debug_reports_its_state() {
        assert_eq!(
            format!("{:?}", EvaluatorAccessSlot::default()),
            "EvaluatorAccessSlot(None)"
        );
        assert_eq!(
            format!("{:?}", EvaluatorAccessSlot(Some(Rc::new(1_u8)))),
            "EvaluatorAccessSlot(Some)"
        );
    }

    #[test]
    fn evaluator_access_is_typed_and_restorable() {
        struct Access(u8);

        let mut context = ThreadContext::new();
        let first = Rc::new(Access(1));
        assert!(context.evaluator_access::<Access>().is_none());
        assert!(
            context
                .replace_evaluator_access(Some(Rc::clone(&first)))
                .is_none()
        );
        assert!(
            context
                .evaluator_access::<Access>()
                .is_some_and(|access| Rc::ptr_eq(&first, &access))
        );
        assert!(context.evaluator_access::<String>().is_none());

        let second = Rc::new(Access(2));
        let previous = context.replace_evaluator_access(Some(second));
        assert_eq!(
            context.evaluator_access::<Access>().map(|access| access.0),
            Some(2)
        );
        context.restore_evaluator_access(previous);
        assert!(
            context
                .evaluator_access::<Access>()
                .is_some_and(|access| Rc::ptr_eq(&first, &access))
        );
    }
}
