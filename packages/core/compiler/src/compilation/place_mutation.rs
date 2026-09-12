#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_setf(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len().is_multiple_of(2) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "setf needs place/value pairs".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let operands = items.get(1..).unwrap_or(&[]);
        if operands.is_empty() {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            return Ok(());
        }
        let (pairs, _) = operands.as_chunks::<2>();
        let mut values = Vec::with_capacity(pairs.len());
        for [_, value_form] in pairs {
            let child = self.reserve_function(None, Vec::new());
            self.compile_expression(child, value_form)?;
            self.emit(child, Instruction::Return, value_form.span)?;
            values.push(child);
        }
        self.emit(
            function,
            Instruction::SetfPlaces {
                invocation: Form::list(items.to_vec(), span),
                values,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_setf_intrinsic_store(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 3 {
            return Err(Self::arity_error(
                items,
                "%SETF-INTRINSIC-STORE",
                "two",
                span,
            ));
        }
        self.compile_expression(function, &items[2])?;
        self.emit(function, Instruction::Setf(items[1].clone()), span)?;
        Ok(())
    }

    pub(crate) fn compile_modify_symbol(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operator: &str,
        arithmetic: &str,
    ) -> Result<(), CompileError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(Self::arity_error(items, operator, "one or two", span));
        }
        let place = items
            .get(1)
            .ok_or_else(|| Self::internal_error(span, "missing modifying place"))?;
        let symbol = match &place.kind {
            FormKind::Atom(_) => Some(Self::symbol_name_info(
                place,
                &format!("{operator} target"),
            )?),
            FormKind::List(_) => None,
            _ => {
                let _ = Self::symbol_name_info(place, &format!("{operator} target"))?;
                unreachable!("symbol_name_info accepted a non-atom place")
            }
        };
        if let Some((name, escaped)) = symbol {
            let temporary = self.fresh_name("MODIFY_DELTA");
            self.emit(function, Instruction::EnterScope, span)?;
            if let Some(delta) = items.get(2) {
                self.compile_expression(function, delta)?;
            } else {
                self.emit(function, Instruction::Constant(Constant::Integer(1)), span)?;
            }
            self.emit(function, Instruction::Define(temporary.clone()), span)?;
            self.emit(function, Instruction::Pop, span)?;
            self.emit(
                function,
                Instruction::FunctionLoad(arithmetic.to_string()),
                place.span,
            )?;
            self.compile_expression(function, place)?;
            self.emit(function, Instruction::Load(temporary), span)?;
            self.emit(function, Instruction::Call(2), span)?;
            let instruction = if escaped {
                Instruction::SetExact(name)
            } else {
                Instruction::Set(name)
            };
            self.emit(function, instruction, place.span)?;
            self.emit(function, Instruction::ExitScope, span)?;
            return Ok(());
        }
        let child = self.reserve_function(None, Vec::new());
        if let Some(delta) = items.get(2) {
            self.compile_expression(child, delta)?;
        } else {
            self.emit(child, Instruction::Constant(Constant::Integer(1)), span)?;
        }
        self.emit(child, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::ModifyPlace {
                invocation: Form::list(items.to_vec(), span),
                delta: child,
                arithmetic: arithmetic.to_string(),
            },
            span,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod setf_tests;
#[cfg(test)]
mod tests;
