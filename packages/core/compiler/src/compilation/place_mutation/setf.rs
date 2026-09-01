#![allow(clippy::wildcard_imports)]
use super::super::*;
use super::setf_emit::emit_pop_if_needed;
use super::setf_validation::validate_setf_items;

impl CompileState {
    pub(crate) fn compile_setf(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let operands = validate_setf_items(items, span)?;
        let (pairs, _) = operands.as_chunks::<2>();
        let pair_count = operands.len() / 2;
        for (index, [place, value_form]) in pairs.iter().enumerate() {
            if self.compile_setf_nth_place(function, place, value_form, index, pair_count)? {
                continue;
            }
            if self.compile_setf_aref_place(function, place, value_form, index, pair_count)? {
                continue;
            }
            let bit_place = match &place.kind {
                FormKind::List(items) if items.len() >= 3 => {
                    Self::symbol_name_info(&items[0], "setf place operator")
                        .ok()
                        .filter(|(name, _)| name == "BIT")
                        .and_then(|_| {
                            Self::symbol_name_info(&items[1], "setf bit target")
                                .ok()
                                .map(|(name, escaped)| {
                                    (items.len() - 2, name, escaped, &items[1], &items[2..])
                                })
                        })
                }
                _ => None,
            };
            if let Some((rank, name, escaped, target, indices)) = bit_place {
                self.compile_expression(function, target)?;
                for index_form in indices {
                    self.compile_expression(function, index_form)?;
                }
                self.compile_expression(function, value_form)?;
                self.emit(
                    function,
                    Instruction::SetfBitDynamic {
                        rank,
                        name,
                        escaped,
                    },
                    place.span,
                )?;
                emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
                continue;
            }
            let element_place = match &place.kind {
                FormKind::List(items) if items.len() == 3 => {
                    Self::symbol_name_info(&items[0], "setf place operator")
                        .ok()
                        .filter(|(name, _)| matches!(name.as_str(), "ELT" | "CHAR" | "SCHAR"))
                        .and_then(|(operator, _)| {
                            Self::symbol_name_info(&items[1], "setf element target")
                                .ok()
                                .map(|(name, escaped)| {
                                    (operator, name, escaped, &items[1], &items[2])
                                })
                        })
                }
                _ => None,
            };
            if let Some((operator, name, escaped, target, index_form)) = element_place {
                self.compile_expression(function, target)?;
                self.compile_expression(function, index_form)?;
                self.compile_expression(function, value_form)?;
                self.emit(
                    function,
                    Instruction::SetfElementDynamic {
                        operator,
                        name,
                        escaped,
                    },
                    place.span,
                )?;
                emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
                continue;
            }
            let subseq_place = match &place.kind {
                FormKind::List(items) if (items.len() == 3 || items.len() == 4) => {
                    Self::symbol_name_info(&items[0], "setf place operator")
                        .ok()
                        .filter(|(name, _)| name == "SUBSEQ")
                        .and_then(|_| {
                            Self::symbol_name_info(&items[1], "setf subseq target")
                                .ok()
                                .map(|(name, escaped)| {
                                    (items.len() == 4, name, escaped, &items[1], &items[2..])
                                })
                        })
                }
                _ => None,
            };
            if let Some((has_end, name, escaped, target, bounds)) = subseq_place {
                self.compile_expression(function, target)?;
                for bound in bounds {
                    self.compile_expression(function, bound)?;
                }
                self.compile_expression(function, value_form)?;
                self.emit(
                    function,
                    Instruction::SetfSubseqDynamic {
                        has_end,
                        name,
                        escaped,
                    },
                    place.span,
                )?;
                emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
                continue;
            }
            let getf_place = match &place.kind {
                FormKind::List(items) if items.len() == 3 => {
                    Self::symbol_name_info(&items[0], "setf place operator")
                        .ok()
                        .filter(|(name, _)| name == "GETF")
                        .and_then(|_| {
                            Self::symbol_name_info(&items[1], "setf getf target")
                                .ok()
                                .map(|(name, escaped)| (name, escaped, &items[1], &items[2]))
                        })
                }
                _ => None,
            };
            if let Some((name, escaped, target, indicator)) = getf_place {
                self.compile_expression(function, target)?;
                self.compile_expression(function, indicator)?;
                self.compile_expression(function, value_form)?;
                self.emit(
                    function,
                    Instruction::SetfGetfDynamic { name, escaped },
                    place.span,
                )?;
                emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
                continue;
            }
            if self.compile_setf_symbol_cell_place(function, place, value_form)? {
                emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
                continue;
            }
            if self.compile_setf_property_place(function, place, value_form)? {
                emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
                continue;
            }
            self.compile_setf_fallback(function, place, value_form)?;
            if index + 1 < pair_count {
                self.emit(function, Instruction::Pop, value_form.span)?;
            }
        }
        Ok(())
    }
}
