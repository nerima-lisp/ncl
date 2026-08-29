#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    #[expect(clippy::too_many_lines)]
    pub(crate) fn compile_do(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        sequential: bool,
    ) -> Result<(), CompileError> {
        let operator = if sequential { "DO*" } else { "DO" };
        let (binding_span, termination_form, termination, parsed) =
            Self::parse_do_form(items, span, operator)?;

        let loop_function = self.reserve_function(None, Vec::new());
        self.emit(loop_function, Instruction::EnterScope, binding_span)?;

        if sequential {
            for (name, escaped, init, _) in &parsed {
                if let Some(init) = init {
                    self.compile_expression(loop_function, init)?;
                } else {
                    self.emit(
                        loop_function,
                        Instruction::Constant(Constant::Nil),
                        binding_span,
                    )?;
                }
                let define = if *escaped {
                    Instruction::DefineExact(name.clone())
                } else {
                    Instruction::Define(name.clone())
                };
                self.emit(loop_function, define, binding_span)?;
                self.emit(loop_function, Instruction::Pop, binding_span)?;
            }
        } else {
            let mut initial_temporaries = Vec::with_capacity(parsed.len());
            for (_, _, init, _) in &parsed {
                if let Some(init) = init {
                    self.compile_expression(loop_function, init)?;
                } else {
                    self.emit(
                        loop_function,
                        Instruction::Constant(Constant::Nil),
                        binding_span,
                    )?;
                }
                let temporary = self.fresh_name("DO_INIT");
                self.emit(
                    loop_function,
                    Instruction::Define(temporary.clone()),
                    binding_span,
                )?;
                self.emit(loop_function, Instruction::Pop, binding_span)?;
                initial_temporaries.push(temporary);
            }
            for ((name, escaped, _, _), temporary) in parsed.iter().zip(initial_temporaries) {
                self.emit(loop_function, Instruction::Load(temporary), binding_span)?;
                let define = if *escaped {
                    Instruction::DefineExact(name.clone())
                } else {
                    Instruction::Define(name.clone())
                };
                self.emit(loop_function, define, binding_span)?;
                self.emit(loop_function, Instruction::Pop, binding_span)?;
            }
        }

        let loop_start = self.instruction_count(loop_function, span)?;
        self.compile_expression(loop_function, &termination[0])?;
        let body_jump = self.emit(
            loop_function,
            Instruction::JumpIfFalse(usize::MAX),
            termination_form.span,
        )?;
        let result_jump = self.emit(
            loop_function,
            Instruction::Jump(usize::MAX),
            termination_form.span,
        )?;
        let body_start = self.instruction_count(loop_function, span)?;
        self.patch_jump(loop_function, body_jump, body_start, span)?;

        self.compile_tagbody_forms(loop_function, span, items.get(3..).unwrap_or(&[]))?;
        self.emit(loop_function, Instruction::Pop, span)?;

        if sequential {
            for (name, escaped, _, step) in &parsed {
                if let Some(step) = step {
                    self.compile_expression(loop_function, step)?;
                    let set = if *escaped {
                        Instruction::SetExact(name.clone())
                    } else {
                        Instruction::Set(name.clone())
                    };
                    self.emit(loop_function, set, binding_span)?;
                    self.emit(loop_function, Instruction::Pop, binding_span)?;
                }
            }
        } else {
            let mut step_temporaries = Vec::with_capacity(parsed.len());
            for (_, _, _, step) in &parsed {
                if let Some(step) = step {
                    self.compile_expression(loop_function, step)?;
                    let temporary = self.fresh_name("DO_STEP");
                    self.emit(
                        loop_function,
                        Instruction::Define(temporary.clone()),
                        binding_span,
                    )?;
                    self.emit(loop_function, Instruction::Pop, binding_span)?;
                    step_temporaries.push(Some(temporary));
                } else {
                    step_temporaries.push(None);
                }
            }
            for ((name, escaped, _, _), temporary) in parsed.iter().zip(step_temporaries) {
                if let Some(temporary) = temporary {
                    self.emit(loop_function, Instruction::Load(temporary), binding_span)?;
                    let set = if *escaped {
                        Instruction::SetExact(name.clone())
                    } else {
                        Instruction::Set(name.clone())
                    };
                    self.emit(loop_function, set, binding_span)?;
                    self.emit(loop_function, Instruction::Pop, binding_span)?;
                }
            }
        }
        self.emit(loop_function, Instruction::Jump(loop_start), span)?;

        let result_start = self.instruction_count(loop_function, span)?;
        self.patch_jump(loop_function, result_jump, result_start, span)?;
        self.compile_sequence(loop_function, termination.get(1..).unwrap_or(&[]))?;
        self.emit(loop_function, Instruction::ExitScope, span)?;
        self.emit(loop_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::Block {
                function: loop_function,
                name: "NIL".to_string(),
            },
            span,
        )?;
        Ok(())
    }
}
