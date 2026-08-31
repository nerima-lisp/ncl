#![allow(clippy::wildcard_imports)]
use super::super::*;

impl CompileState {
    pub(super) fn compile_setf_fallback(
        &mut self,
        function: FunctionId,
        place: &Form,
        value_form: &Form,
    ) -> Result<(), CompileError> {
        let list_place = match &place.kind {
            FormKind::List(items) if items.len() == 2 => {
                let operator = Self::symbol_name_info(&items[0], "setf place operator")
                    .ok()
                    .map(|(name, _)| name);
                operator.and_then(|operator| {
                    if matches!(operator.as_str(), "CAR" | "FIRST" | "CDR" | "REST") {
                        Self::symbol_name_info(&items[1], "setf list target")
                            .ok()
                            .map(|(name, escaped)| (operator, name, escaped, &items[1]))
                    } else {
                        None
                    }
                })
            }
            _ => None,
        };
        if let Some((operator, name, escaped, target)) = list_place {
            self.compile_expression(function, target)?;
            self.compile_expression(function, value_form)?;
            self.emit(
                function,
                Instruction::SetfList {
                    operator,
                    name,
                    escaped,
                },
                place.span,
            )?;
        } else {
            self.compile_expression(function, value_form)?;
            let instruction = match Self::symbol_name_info(place, "setf place") {
                Ok((name, escaped)) if escaped => Instruction::SetExact(name),
                Ok((name, _)) => Instruction::Set(name),
                Err(_) => Instruction::Setf(place.clone()),
            };
            self.emit(function, instruction, place.span)?;
        }
        Ok(())
    }
}
