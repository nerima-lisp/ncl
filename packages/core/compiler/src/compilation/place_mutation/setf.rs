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
            let nth_place = match &place.kind {
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
                            Some(FormKind::Atom(atom)) => {
                                match crate::helpers::literal_constant(&atom) {
                                    Some(Constant::Integer(value)) if value >= 0 => {
                                        Some(value as usize)
                                    }
                                    _ => None,
                                }
                            }
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
            };
            if let Some((nth_index, name, escaped, index_form, target)) = nth_place {
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
                emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
                continue;
            }
            let aref_place = match &place.kind {
                FormKind::List(items) if items.len() >= 3 => {
                    let operator = Self::symbol_name_info(&items[0], "setf place operator")
                        .ok()
                        .map(|(name, _)| name);
                    operator.and_then(|operator| {
                        if !matches!(operator.as_str(), "AREF" | "SVREF" | "ROW-MAJOR-AREF") {
                            return None;
                        }
                        Self::symbol_name_info(&items[1], "setf aref target")
                            .ok()
                            .map(|(name, escaped)| {
                                (
                                    operator,
                                    items.len() - 2,
                                    name,
                                    escaped,
                                    &items[1],
                                    &items[2..],
                                )
                            })
                    })
                }
                _ => None,
            };
            if let Some((operator, rank, name, escaped, target, indices)) = aref_place {
                self.compile_expression(function, target)?;
                for index_form in indices {
                    self.compile_expression(function, index_form)?;
                }
                self.compile_expression(function, value_form)?;
                self.emit(
                    function,
                    Instruction::SetfArefDynamic {
                        rank,
                        operator,
                        name,
                        escaped,
                    },
                    place.span,
                )?;
                emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
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
