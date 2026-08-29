#![allow(clippy::wildcard_imports)]
use crate::*;

mod auxiliary;
mod default;
mod keyword;
mod lambda_list;
mod lambda_list_markers;
mod optional;
mod parameter_section;
mod pattern;

impl CompileState {
    pub(crate) fn compile_destructuring_bind(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(
                items,
                "DESTRUCTURING-BIND",
                "two or more",
                span,
            ));
        }
        let mut seen = HashSet::new();
        let specification = match &items[1].kind {
            FormKind::List(_) => {
                DestructureSpec::LambdaList(self.compile_destructuring_lambda_list(&items[1])?)
            }
            _ => {
                DestructureSpec::Pattern(Self::compile_destructuring_pattern(&items[1], &mut seen)?)
            }
        };
        self.emit(function, Instruction::EnterScope, items[1].span)?;
        self.compile_expression(function, &items[2])?;
        self.emit(
            function,
            Instruction::Destructure(specification),
            items[1].span,
        )?;
        self.compile_sequence(function, items.get(3..).unwrap_or(&[]))?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }
}
