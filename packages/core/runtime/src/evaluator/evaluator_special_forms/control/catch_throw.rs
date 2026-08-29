use ncl_syntax::Form;

use crate::error::ThrowTag;
use crate::{Environment, ReturnValue, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_catch(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity("catch", "at least one", 0));
        }

        let tag = self.eval_values_in(&items[1], environment)?.primary_value();
        match self.eval_sequence_values(&items[2..], environment) {
            Ok(value) => Ok(value),
            Err(RuntimeError::Throw {
                tag: thrown_tag,
                value,
                ..
            }) if thrown_tag.matches(&tag) => Ok(value.into_value()),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn special_throw(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(Self::arity("throw", "two", items.len().saturating_sub(1)));
        }

        let tag = self.eval_values_in(&items[1], environment)?.primary_value();
        let value = self.eval_values_in(&items[2], environment)?;
        Err(RuntimeError::Throw {
            tag: ThrowTag::new(tag),
            value: ReturnValue::new(value),
            span: Some(items[0].span),
        })
    }

    pub(crate) fn special_unwind_protect(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                "unwind-protect",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }

        let protected = self.eval_values_in(&items[1], environment);
        let cleanup = self.eval_sequence_values(&items[2..], environment);
        match cleanup {
            Ok(_) => protected,
            Err(error) => Err(error),
        }
    }
}
