#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_setf(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "setf needs place/value pairs".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let operands = items.get(1..).unwrap_or(&[]);
        let (pairs, _) = operands.as_chunks::<2>();
        let pair_count = operands.len() / 2;
        for (index, [place, value_form]) in pairs.iter().enumerate() {
            let nth_place = match &place.kind {
                FormKind::List(items) if items.len() == 3 => {
                    let operator = Self::symbol_name_info(&items[0], "setf place operator")
                        .ok()
                        .map(|(name, _)| name);
                    operator.and_then(|operator| {
                        if operator != "NTH" {
                            return None;
                        }
                        let index = match &items[1].kind {
                            FormKind::Atom(atom) => match crate::helpers::literal_constant(atom) {
                                Some(Constant::Integer(value)) if value >= 0 => {
                                    Some(value as usize)
                                }
                                _ => None,
                            },
                            _ => None,
                        };
                        Self::symbol_name_info(&items[2], "setf nth target")
                            .ok()
                            .map(|(name, escaped)| (index, name, escaped, &items[1], &items[2]))
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
                if index + 1 < pair_count {
                    self.emit(function, Instruction::Pop, value_form.span)?;
                }
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
                if index + 1 < pair_count {
                    self.emit(function, Instruction::Pop, value_form.span)?;
                }
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
                if index + 1 < pair_count {
                    self.emit(function, Instruction::Pop, value_form.span)?;
                }
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
                if index + 1 < pair_count {
                    self.emit(function, Instruction::Pop, value_form.span)?;
                }
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
                if index + 1 < pair_count {
                    self.emit(function, Instruction::Pop, value_form.span)?;
                }
                continue;
            }
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
            if index + 1 < pair_count {
                self.emit(function, Instruction::Pop, value_form.span)?;
            }
        }
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
        let (name, escaped) = Self::symbol_name_info(place, &format!("{operator} target"))?;
        self.emit(
            function,
            Instruction::FunctionLoad(arithmetic.to_string()),
            place.span,
        )?;
        self.compile_expression(function, place)?;
        if let Some(delta) = items.get(2) {
            self.compile_expression(function, delta)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Integer(1)), span)?;
        }
        self.emit(function, Instruction::Call(2), span)?;
        self.emit(
            function,
            if escaped {
                Instruction::SetExact(name)
            } else {
                Instruction::Set(name)
            },
            place.span,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
