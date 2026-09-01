#[allow(clippy::wildcard_imports)]
use super::*;

mod native_places;
mod rotate_shift;

impl CompileState {
    pub(super) fn compile_load_time_value(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity_error(
                items,
                "LOAD-TIME-VALUE",
                "one or two",
                span,
            ));
        }
        self.emit(
            function,
            Instruction::LoadTimeValue(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_defstruct(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "DEFSTRUCT", "at least one", span));
        }
        self.emit(
            function,
            Instruction::Defstruct(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_defclass(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 4 {
            return Err(Self::arity_error(items, "DEFCLASS", "at least three", span));
        }
        self.emit(
            function,
            Instruction::Defclass(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_defgeneric(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, "DEFGENERIC", "at least two", span));
        }
        self.emit(
            function,
            Instruction::Defgeneric(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_defmethod(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, "DEFMETHOD", "at least two", span));
        }
        self.emit(
            function,
            Instruction::Defmethod(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_defsetf(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 3 {
            return Err(Self::arity_error(items, "DEFSETF", "two", span));
        }
        self.emit(
            function,
            Instruction::Defsetf(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_defconstant(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(items.len() == 3 || items.len() == 4) {
            return Err(Self::arity_error(
                items,
                "DEFCONSTANT",
                "two or three",
                span,
            ));
        }
        self.emit(
            function,
            Instruction::Defconstant(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_define_symbol_macro(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 3 {
            return Err(Self::arity_error(items, "DEFINE-SYMBOL-MACRO", "two", span));
        }
        self.emit(
            function,
            Instruction::DefineSymbolMacro(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_define_setf_expander(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 4 {
            return Err(Self::arity_error(
                items,
                "DEFINE-SETF-EXPANDER",
                "at least three",
                span,
            ));
        }
        self.emit(
            function,
            Instruction::DefineSetfExpander(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_define_modify_macro(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 4 {
            return Err(Self::arity_error(
                items,
                "DEFINE-MODIFY-MACRO",
                "at least three",
                span,
            ));
        }
        self.emit(
            function,
            Instruction::DefineModifyMacro(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_get_setf_expansion(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity_error(
                items,
                "GET-SETF-EXPANSION",
                "one or two",
                span,
            ));
        }
        self.emit(
            function,
            Instruction::GetSetfExpansion(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_psetf(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(Self::arity_error(
                items,
                "PSETF",
                "one or more place/value pairs",
                span,
            ));
        }
        self.emit(
            function,
            Instruction::Psetf(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

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
        self.emit(
            function,
            Instruction::Quote(Form::list(items.to_vec(), span)),
            span,
        )?;
        self.emit(function, Instruction::Eval(span), span)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
