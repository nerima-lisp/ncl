#![allow(clippy::wildcard_imports)]
use super::super::*;
use super::setf_emit::emit_pop_if_needed;

impl CompileState {
    pub(super) fn compile_setf_nth_place(
        &mut self,
        function: FunctionId,
        place: &Form,
        value_form: &Form,
        pair_index: usize,
        pair_count: usize,
    ) -> Result<bool, CompileError> {
        let Some((nth_index, name, escaped, index_form, target)) = (match &place.kind {
            FormKind::List(items) if (items.len() == 2 || items.len() == 3) => {
                let operator = Self::symbol_name_info(&items[0], "setf place operator")
                    .ok()
                    .map(|(name, _)| name);
                operator.and_then(|operator| {
                    let fixed_index = match operator.as_str() {
                        "NTH" => None,
                        "SECOND" => Some(1),
                        "THIRD" => Some(2),
                        "FOURTH" => Some(3),
                        "FIFTH" => Some(4),
                        "SIXTH" => Some(5),
                        "SEVENTH" => Some(6),
                        "EIGHTH" => Some(7),
                        "NINTH" => Some(8),
                        "TENTH" => Some(9),
                        _ => return None,
                    };
                    let index_form = items.get(1);
                    let target_form = items.get(if fixed_index.is_some() { 1 } else { 2 })?;
                    let index = match index_form.map(|form| &form.kind) {
                        Some(FormKind::Atom(atom)) => match crate::helpers::literal_constant(&atom)
                        {
                            Some(Constant::Integer(value)) if value >= 0 => Some(value as usize),
                            _ => None,
                        },
                        _ => None,
                    };
                    Self::symbol_name_info(target_form, "setf nth target")
                        .ok()
                        .map(|(name, escaped)| {
                            (
                                fixed_index.or(index),
                                name,
                                escaped,
                                index_form.unwrap_or(target_form),
                                target_form,
                            )
                        })
                })
            }
            _ => None,
        }) else {
            return Ok(false);
        };
        if let Some(nth_index) = nth_index {
            self.compile_expression(function, target)?;
            self.compile_expression(function, value_form)?;
            self.emit(
                function,
                Instruction::SetfNth {
                    index: nth_index,
                    name,
                    escaped,
                },
                place.span,
            )?;
        } else {
            self.compile_expression(function, index_form)?;
            self.compile_expression(function, target)?;
            self.compile_expression(function, value_form)?;
            self.emit(
                function,
                Instruction::SetfNthDynamic { name, escaped },
                place.span,
            )?;
        }
        emit_pop_if_needed(self, function, pair_index, pair_count, value_form.span)?;
        Ok(true)
    }
}
