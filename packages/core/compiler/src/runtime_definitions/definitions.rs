#[allow(clippy::wildcard_imports)]
use super::super::*;

impl CompileState {
    pub(crate) fn compile_load_time_value(
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

    pub(crate) fn compile_defstruct(
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

    pub(crate) fn compile_deftype(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 4 {
            return Err(Self::arity_error(items, "DEFTYPE", "three", span));
        }
        self.emit(
            function,
            Instruction::Deftype(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_defclass(
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

    pub(crate) fn compile_define_condition(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 4 {
            return Err(Self::arity_error(
                items,
                "DEFINE-CONDITION",
                "at least three",
                span,
            ));
        }
        self.emit(
            function,
            Instruction::DefineCondition(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_defgeneric(
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

    pub(crate) fn compile_defmethod(
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

    pub(crate) fn compile_defconstant(
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

    pub(crate) fn compile_define_symbol_macro(
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
}
