#[allow(clippy::wildcard_imports)]
use super::super::*;

impl CompileState {
    pub(crate) fn compile_defsetf(
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

    pub(crate) fn compile_define_setf_expander(
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

    pub(crate) fn compile_define_modify_macro(
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

    pub(crate) fn compile_get_setf_expansion(
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

    pub(crate) fn compile_psetf(
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
        if items.len() == 3 {
            return self.compile_setf(function, span, items);
        }
        if let Some(names) = items[1..]
            .chunks_exact(2)
            .map(|pair| {
                matches!(pair[0].kind, FormKind::Atom(_))
                    .then(|| Self::symbol_name_info(&pair[0], "PSETF place"))
            })
            .collect::<Option<Result<Vec<_>, _>>>()
            .transpose()?
        {
            for pair in items[1..].chunks_exact(2) {
                self.compile_expression(function, &pair[1])?;
            }
            self.emit(function, Instruction::PsetfSymbols(names), span)?;
            return Ok(());
        }
        if let Some(places) = items[1..]
            .chunks_exact(2)
            .map(|pair| {
                let mut accessors = Vec::new();
                let mut target = &pair[0];
                while let Some((accessor, next_target)) = crate::helpers::list_accessor_target(target) {
                    accessors.push(accessor);
                    target = next_target;
                }
                if accessors.is_empty() {
                    return None;
                }
                let (name, escaped) = Self::symbol_name_info(target, "PSETF list target").ok()?;
                accessors.reverse();
                Some((accessors, name, escaped))
            })
            .collect::<Option<Vec<_>>>()
        {
            for pair in items[1..].chunks_exact(2) {
                self.compile_expression(function, &pair[1])?;
            }
            self.emit(function, Instruction::PsetfList(places), span)?;
            return Ok(());
        }
        self.emit(
            function,
            Instruction::Psetf(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }
}
