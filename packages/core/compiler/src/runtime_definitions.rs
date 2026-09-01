#[allow(clippy::wildcard_imports)]
use super::*;

mod native_places;
mod rotate_shift;
mod definitions;
mod setf;

impl CompileState {
    pub(super) fn compile_runtime_definition(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(
                items,
                "runtime definition",
                "at least one",
                span,
            ));
        }
        if let Some(result) = self.compile_native_push_pop(function, span, items)? {
            return Ok(result);
        }
        if let Some(result) = self.compile_native_rotate_shift(function, span, items)? {
            return Ok(result);
        }
        if let Some(result) = self.compile_native_remf(function, span, items)? {
            return Ok(result);
        }
        self.emit(
            function,
            Instruction::RuntimeMutation(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
