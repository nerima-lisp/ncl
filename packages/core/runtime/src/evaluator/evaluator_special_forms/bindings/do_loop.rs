use super::{Environment, Form, FormKind, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_do(
        &self,
        items: &[Form],
        environment: &Environment,
        sequential: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if sequential { "do*" } else { "do" };
        if items.len() < 3 {
            return Err(Self::arity(
                operator,
                "at least two",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(binding_forms) = &items[1].kind else {
            return Err(Self::invalid("do bindings must be a list", items[1].span));
        };
        let FormKind::List(termination) = &items[2].kind else {
            return Err(Self::invalid(
                "do termination must be a list",
                items[2].span,
            ));
        };
        if termination.is_empty() {
            return Err(Self::invalid(
                "do termination needs an end test",
                items[2].span,
            ));
        }

        let bindings = Self::parse_do_bindings(binding_forms)?;

        let target = self.fresh_block_target();
        let block_environment = environment.child();
        block_environment.define_block("NIL", target);
        let local = block_environment.child();
        let _dynamic_guard = self.dynamic_guard();

        let initialization =
            self.initialize_do_bindings(&bindings, sequential, &local, &block_environment);
        match initialization {
            Ok(()) => {}
            Err(RuntimeError::ReturnFrom {
                target: Some(return_target),
                value,
                ..
            }) if return_target == target => return Ok(value.into_value()),
            Err(error) => return Err(error),
        }

        loop {
            let iteration = (|| -> Result<Option<Value>, RuntimeError> {
                let test = self.eval_in(&termination[0], &local)?;
                if test.is_truthy() {
                    return Ok(Some(self.eval_sequence_values(&termination[1..], &local)?));
                }

                self.eval_tagbody_forms(&items[3..], &local)?;
                self.advance_do_bindings(&bindings, sequential, &local)?;
                Ok(None)
            })();

            match iteration {
                Ok(Some(value)) => return Ok(value),
                Ok(None) => {}
                Err(RuntimeError::ReturnFrom {
                    target: Some(return_target),
                    value,
                    ..
                }) if return_target == target => return Ok(value.into_value()),
                Err(error) => return Err(error),
            }
        }
    }
}
