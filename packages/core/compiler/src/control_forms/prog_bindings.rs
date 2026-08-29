#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(super) fn compile_sequential_prog_bindings(
        &mut self,
        function: FunctionId,
        span: Span,
        bindings: &[(String, bool, Option<Form>)],
    ) -> Result<(), CompileError> {
        for (name, escaped, init) in bindings {
            self.compile_prog_initializer(function, span, init.as_ref())?;
            self.emit_prog_definition(function, span, name, *escaped)?;
        }
        Ok(())
    }

    pub(super) fn compile_parallel_prog_bindings(
        &mut self,
        function: FunctionId,
        span: Span,
        bindings: &[(String, bool, Option<Form>)],
    ) -> Result<(), CompileError> {
        let mut temporaries = Vec::with_capacity(bindings.len());
        for (_, _, init) in bindings {
            self.compile_prog_initializer(function, span, init.as_ref())?;
            let temporary = self.fresh_name("PROG_INIT");
            self.emit(function, Instruction::Define(temporary.clone()), span)?;
            self.emit(function, Instruction::Pop, span)?;
            temporaries.push(temporary);
        }
        for ((name, escaped, _), temporary) in bindings.iter().zip(temporaries) {
            self.emit(function, Instruction::Load(temporary), span)?;
            self.emit_prog_definition(function, span, name, *escaped)?;
        }
        Ok(())
    }

    fn compile_prog_initializer(
        &mut self,
        function: FunctionId,
        span: Span,
        init: Option<&Form>,
    ) -> Result<(), CompileError> {
        if let Some(init) = init {
            self.compile_expression(function, init)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        Ok(())
    }

    fn emit_prog_definition(
        &mut self,
        function: FunctionId,
        span: Span,
        name: &str,
        escaped: bool,
    ) -> Result<(), CompileError> {
        let instruction = if escaped {
            Instruction::DefineExact(name.to_string())
        } else {
            Instruction::Define(name.to_string())
        };
        self.emit(function, instruction, span)?;
        self.emit(function, Instruction::Pop, span)?;
        Ok(())
    }
}
