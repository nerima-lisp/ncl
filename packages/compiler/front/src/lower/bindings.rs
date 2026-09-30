//! Lexical binding and call-form lowering for the IR v2 path.

use std::collections::BTreeMap;

use ncl_ir::{
    Constant, Convert, FunctionId, HandlerKind, HandlerRegion, HandlerRegionId, OpKind, Terminator,
    Ty, ValueId,
};

use crate::ast::Expr;
use crate::declaration::Declaration;
use crate::symbols::SymbolRef;

use super::super::capture;
use super::super::env::{FunctionEntry, Slot};
use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::Context;
use super::analysis::mentions_exit;
use super::params::{bind_captures, bind_let, bind_required, lambda_params};

type RecursiveCaptures = (Vec<(SymbolRef, Slot)>, Vec<SymbolRef>);

/// The names a `let`/`let*`'s declarations mark special: those declared
/// `(special name*)` locally in the body, and those the front end folded in
/// from a prior global proclamation (`defvar`, `defparameter`, `declaim`, or
/// `proclaim`).
fn special_names(declarations: &[Declaration]) -> Vec<SymbolRef> {
    declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::Special(names) => Some(names.iter().cloned()),
            _ => None,
        })
        .flatten()
        .collect()
}

impl Context<'_> {
    pub(super) fn lower_progv(
        &mut self,
        f: &mut FunctionLowerer,
        symbols: &Expr,
        values: &Expr,
        body: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let symbols = self.lower_expr(f, symbols)?;
        let values = self.lower_expr(f, values)?;
        let result = f.fresh_value();
        let region_id = HandlerRegionId(self.next_region);
        self.next_region += 1;
        let start = f.current_block();
        self.enter(f, region_id)?;
        let value = self.lower_body(f, body)?;
        let protected = self
            .blocks
            .iter()
            .copied()
            .filter(|block| block.0 >= start.0)
            .collect::<Vec<_>>();
        let body_end = f.current_block();
        let normal_path = !f.is_terminated();
        let merge = self.block(f, vec![(Ty::Word, result)]);
        if normal_path {
            f.position(body_end)?;
            self.leave(f, region_id)?;
            f.terminate(Terminator::Jump {
                target: merge,
                args: vec![value],
            })?;
        }
        let handler = self.block(f, Vec::new());
        self.leave(f, region_id)?;
        let restored_value = f.nil()?;
        f.terminate(Terminator::Return {
            values: vec![restored_value],
        })?;
        self.regions.push(HandlerRegion {
            id: region_id,
            kind: HandlerKind::Progv,
            protected,
            handler,
            cleanup: None,
            catch_tag: None,
            binding_targets: vec![symbols, values],
            depth: 0,
            parent: None,
        });
        f.position(merge)?;
        Ok(result)
    }

    pub(super) fn lower_let(
        &mut self,
        f: &mut FunctionLowerer,
        sequential: bool,
        bindings: &[crate::ast::LetBinding],
        declarations: &[Declaration],
        body: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let specials = special_names(declarations);
        let analysis = capture::analyze(body);
        f.env().push();
        let value = if sequential {
            self.lower_let_sequential(f, bindings, &specials, &analysis, body)?
        } else {
            let mut evaluated = Vec::with_capacity(bindings.len());
            for binding in bindings {
                let value = match binding.value.as_ref() {
                    Some(form) => self.lower_expr(f, form)?,
                    None => f.nil()?,
                };
                evaluated.push(value);
            }
            self.lower_let_parallel(f, bindings, &evaluated, &specials, &analysis, body)?
        };
        f.env().pop();
        Ok(value)
    }

    /// Lower a `let*`'s remaining bindings and body, evaluating each init
    /// form immediately before installing its binding (so a later binding's
    /// init form observes an earlier special's new dynamic value, matching
    /// sequential `let*` semantics) and dynamically rebinding every name
    /// `declarations` marks special rather than giving it a lexical slot.
    fn lower_let_sequential(
        &mut self,
        f: &mut FunctionLowerer,
        bindings: &[crate::ast::LetBinding],
        specials: &[SymbolRef],
        analysis: &capture::Analysis,
        body: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let Some((binding, rest)) = bindings.split_first() else {
            return self.lower_body(f, body);
        };
        let value = match binding.value.as_ref() {
            Some(form) => self.lower_expr(f, form)?,
            None => f.nil()?,
        };
        if specials.contains(&binding.name) {
            self.lower_dynamic_binding(f, &binding.name, value, move |ctx, f| {
                ctx.lower_let_sequential(f, rest, specials, analysis, body)
            })
        } else {
            bind_let(f, &binding.name, value, analysis)?;
            self.lower_let_sequential(f, rest, specials, analysis, body)
        }
    }

    /// Lower a `let`'s remaining bindings and body from already-evaluated
    /// init values (`let` evaluates every init form before any binding takes
    /// effect), dynamically rebinding every name `declarations` marks special
    /// rather than giving it a lexical slot.
    fn lower_let_parallel(
        &mut self,
        f: &mut FunctionLowerer,
        bindings: &[crate::ast::LetBinding],
        values: &[ValueId],
        specials: &[SymbolRef],
        analysis: &capture::Analysis,
        body: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let (Some((binding, rest_bindings)), Some((&value, rest_values))) =
            (bindings.split_first(), values.split_first())
        else {
            return self.lower_body(f, body);
        };
        if specials.contains(&binding.name) {
            self.lower_dynamic_binding(f, &binding.name, value, move |ctx, f| {
                ctx.lower_let_parallel(f, rest_bindings, rest_values, specials, analysis, body)
            })
        } else {
            bind_let(f, &binding.name, value, analysis)?;
            self.lower_let_parallel(f, rest_bindings, rest_values, specials, analysis, body)
        }
    }

    /// Dynamically rebind one special variable's global value cell around
    /// `continuation`, restoring the previous value (bound or unbound) when
    /// leaving normally and when a non-local exit unwinds past this point.
    ///
    /// This reuses exactly the `progv` handler-region machinery (the same
    /// GC-safe binding stack, established via [`HandlerKind::Progv`] and the
    /// `EnterProgv`/`LeaveProgv` runtime functions): a `let`/`let*` binding
    /// that names a special variable behaves precisely like a one-variable
    /// `progv` wrapped around the rest of the form.
    fn lower_dynamic_binding(
        &mut self,
        f: &mut FunctionLowerer,
        name: &SymbolRef,
        value: ValueId,
        continuation: impl FnOnce(&mut Self, &mut FunctionLowerer) -> Result<ValueId, LowerError>,
    ) -> Result<ValueId, LowerError> {
        let symbol = f.symbol(name)?;
        let nil = f.nil()?;
        f.safepoint()?;
        let symbols = f.one(
            OpKind::Builtin {
                name: "CONS".to_owned(),
                args: vec![symbol, nil],
            },
            Ty::Word,
        )?;
        f.safepoint()?;
        let values = f.one(
            OpKind::Builtin {
                name: "CONS".to_owned(),
                args: vec![value, nil],
            },
            Ty::Word,
        )?;
        let result = f.fresh_value();
        let region_id = HandlerRegionId(self.next_region);
        self.next_region += 1;
        let start = f.current_block();
        self.enter(f, region_id)?;
        let inner = continuation(self, f)?;
        let protected = self
            .blocks
            .iter()
            .copied()
            .filter(|block| block.0 >= start.0)
            .collect::<Vec<_>>();
        let body_end = f.current_block();
        let normal_path = !f.is_terminated();
        let merge = self.block(f, vec![(Ty::Word, result)]);
        if normal_path {
            f.position(body_end)?;
            self.leave(f, region_id)?;
            f.terminate(Terminator::Jump {
                target: merge,
                args: vec![inner],
            })?;
        }
        let handler = self.block(f, Vec::new());
        self.leave(f, region_id)?;
        let restored_value = f.nil()?;
        f.terminate(Terminator::Return {
            values: vec![restored_value],
        })?;
        self.regions.push(HandlerRegion {
            id: region_id,
            kind: HandlerKind::Progv,
            protected,
            handler,
            cleanup: None,
            catch_tag: None,
            binding_targets: vec![symbols, values],
            depth: 0,
            parent: None,
        });
        f.position(merge)?;
        Ok(result)
    }

    pub(super) fn lower_setq(
        &mut self,
        f: &mut FunctionLowerer,
        pairs: &[(SymbolRef, Expr)],
    ) -> Result<ValueId, LowerError> {
        let mut last = None;
        for (name, form) in pairs {
            let value = self.lower_expr(f, form)?;
            match f.env().lookup_variable(name) {
                Some(Slot::Cell(cell)) => f.none(OpKind::StoreField {
                    object: cell,
                    field: 0,
                    value,
                })?,
                Some(Slot::Value(_)) => {
                    f.env().rebind_variable(name, Slot::Value(value));
                }
                None => {
                    let symbol = f.symbol(name)?;
                    f.none(OpKind::StoreField {
                        object: symbol,
                        field: u32::try_from(ncl_object::symbol_offset::VALUE).map_err(|_| {
                            LowerError::Ir {
                                detail: "symbol value offset does not fit u32".to_owned(),
                            }
                        })?,
                        value,
                    })?;
                }
            }
            last = Some(value);
        }
        last.map_or_else(|| f.nil(), Ok)
    }

    pub(super) fn lower_multiple_value_call(
        &mut self,
        f: &mut FunctionLowerer,
        function: &Expr,
        arguments: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let callee = self.lower_expr(f, function)?;
        let mut values = Vec::with_capacity(arguments.len());
        for argument in arguments {
            self.lower_expr(f, argument)?;
            let helper = f.symbol(&SymbolRef::interned("NCL-EXT", "CAPTURE-MULTIPLE-VALUES"))?;
            let helper = f.one(
                OpKind::LoadField {
                    object: helper,
                    field: u32::try_from(ncl_object::symbol_offset::FUNCTION).map_err(|_| {
                        LowerError::Ir {
                            detail: "function symbol offset does not fit u32".to_owned(),
                        }
                    })?,
                },
                Ty::Word,
            )?;
            let zero = f.fixnum(0)?;
            let zero = f.one(
                OpKind::Convert {
                    op: Convert::I64ToWord,
                    value: zero,
                },
                Ty::Word,
            )?;
            f.safepoint()?;
            values.push(f.one(
                OpKind::CallClosure {
                    closure: helper,
                    args: vec![zero],
                    named_symbol: None,
                },
                Ty::Word,
            )?);
        }
        let helper = f.symbol(&SymbolRef::interned("NCL-EXT", "MULTIPLE-VALUE-CALL-LIST"))?;
        let helper = f.one(
            OpKind::LoadField {
                object: helper,
                field: u32::try_from(ncl_object::symbol_offset::FUNCTION).map_err(|_| {
                    LowerError::Ir {
                        detail: "function symbol offset does not fit u32".to_owned(),
                    }
                })?,
            },
            Ty::Word,
        )?;
        values.insert(0, callee);
        let raw_argc = f.fixnum(i64::try_from(values.len()).map_err(|_| LowerError::Ir {
            detail: "argument count does not fit i64".to_owned(),
        })?)?;
        let argc = f.one(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: raw_argc,
            },
            Ty::Word,
        )?;
        let mut call_args = vec![argc];
        call_args.extend(values);
        f.safepoint()?;
        f.one(
            OpKind::CallClosure {
                closure: helper,
                args: call_args,
                named_symbol: None,
            },
            Ty::Word,
        )
    }

    pub(super) fn lower_local_functions(
        &mut self,
        f: &mut FunctionLowerer,
        definitions: &[crate::ast::LocalFunction],
        body: &[Expr],
        recursive: bool,
    ) -> Result<ValueId, LowerError> {
        f.env().push();
        if recursive {
            return self.lower_recursive_functions(f, definitions, body);
        }
        for definition in definitions {
            let closure = self.lower_lambda_value(f, &definition.lambda)?;
            f.env().bind_function(FunctionEntry {
                name: definition.name.clone(),
                callee: closure,
            });
        }
        let value = self.lower_body(f, body)?;
        f.env().pop();
        Ok(value)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "keeps recursive closure construction together"
    )]
    fn lower_recursive_functions(
        &mut self,
        f: &mut FunctionLowerer,
        definitions: &[crate::ast::LocalFunction],
        body: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let ids = definitions
            .iter()
            .map(|_| self.module.fresh_function())
            .collect::<Vec<FunctionId>>();
        let captures = self.recursive_captures(f, definitions);
        for ((definition, id), (variable_captures, function_captures)) in definitions
            .iter()
            .zip(ids.iter().copied())
            .zip(captures.iter())
        {
            let params = lambda_params(&definition.lambda.lambda_list)?;
            let mut nested =
                FunctionLowerer::new(id, format!("lambda-{id:?}"), params, vec![Ty::Word]);
            bind_captures(&mut nested, variable_captures)?;
            for (index, name) in function_captures.iter().enumerate() {
                let value = nested.one(
                    OpKind::LoadCapture {
                        index: u8::try_from(variable_captures.len() + index).map_err(|_| {
                            LowerError::Ir {
                                detail: "function capture index does not fit u8".to_owned(),
                            }
                        })?,
                    },
                    Ty::Word,
                )?;
                nested.env().bind_function(FunctionEntry {
                    name: name.clone(),
                    callee: value,
                });
            }
            bind_required(&mut nested, &definition.lambda.lambda_list, 1)?;
            let mut child = Context::with_targets(self.module, self.targets.clone());
            child.bind_optional(&mut nested, &definition.lambda.lambda_list, 1)?;
            child.bind_rest_and_keys(&mut nested, &definition.lambda.lambda_list)?;
            child.bind_aux(&mut nested, &definition.lambda.lambda_list)?;
            for ((target, target_id), (target_variables, target_functions)) in definitions
                .iter()
                .zip(ids.iter().copied())
                .zip(captures.iter())
            {
                let mut capture_values = target_variables
                    .iter()
                    .map(|(name, _slot)| match nested.env().lookup_variable(name) {
                        Some(Slot::Value(value)) => Ok(value),
                        Some(Slot::Cell(cell)) => Ok(cell),
                        None => Err(LowerError::Ir {
                            detail: format!("recursive function capture is unavailable: {name}"),
                        }),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                capture_values.extend(
                    target_functions
                        .iter()
                        .map(|name| {
                            nested
                                .env()
                                .lookup_function(name)
                                .map(|entry| entry.callee)
                                .ok_or_else(|| LowerError::Ir {
                                    detail: format!(
                                        "recursive function capture is unavailable: {name}"
                                    ),
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                );
                let entry = nested.word_constant(Constant::FunctionEntry(target_id))?;
                let closure = nested.one(
                    OpKind::MakeClosure {
                        entry,
                        captures: capture_values,
                    },
                    Ty::Word,
                )?;
                nested.env().bind_function(FunctionEntry {
                    name: target.name.clone(),
                    callee: closure,
                });
            }
            let value = child.lower_body(&mut nested, &definition.lambda.body)?;
            if !nested.is_terminated() {
                nested.return_value(value)?;
            }
            let mut function = nested.into_function();
            function.handler_regions = child.regions;
            child.module.push_function(function);

            let entry = f.word_constant(Constant::FunctionEntry(id))?;
            let mut capture_values = variable_captures
                .iter()
                .map(|(_, slot)| match slot {
                    Slot::Value(value) => Ok(*value),
                    Slot::Cell(cell) => Ok(*cell),
                })
                .collect::<Result<Vec<_>, LowerError>>()?;
            capture_values.extend(
                function_captures
                    .iter()
                    .map(|name| {
                        f.env()
                            .lookup_function(name)
                            .map(|entry| entry.callee)
                            .ok_or_else(|| LowerError::Ir {
                                detail: format!(
                                    "recursive function capture is unavailable: {name}"
                                ),
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            let closure = f.one(
                OpKind::MakeClosure {
                    entry,
                    captures: capture_values,
                },
                Ty::Word,
            )?;
            f.env().bind_function(FunctionEntry {
                name: definition.name.clone(),
                callee: closure,
            });
        }
        let value = self.lower_body(f, body)?;
        f.env().pop();
        Ok(value)
    }

    fn recursive_captures(
        &self,
        f: &mut FunctionLowerer,
        definitions: &[crate::ast::LocalFunction],
    ) -> Vec<RecursiveCaptures> {
        let local_indices = definitions
            .iter()
            .enumerate()
            .map(|(index, definition)| (definition.name.clone(), index))
            .collect::<BTreeMap<_, _>>();
        let mut capture_names = definitions
            .iter()
            .map(|definition| capture::free_names(&definition.lambda).variables)
            .collect::<Vec<_>>();
        for (index, definition) in definitions.iter().enumerate() {
            for target in &self.targets {
                if mentions_exit(&definition.lambda.body, &target.name)
                    && let Some(names) = capture_names.get_mut(index)
                {
                    names.insert(target.capture.clone());
                }
            }
        }
        let mut function_names = definitions
            .iter()
            .map(|definition| capture::free_names(&definition.lambda).functions)
            .collect::<Vec<_>>();
        let references = function_names.clone();
        let mut changed = true;
        while changed {
            changed = false;
            for (index, names) in references.iter().enumerate() {
                for name in names.clone() {
                    let Some(&target) = local_indices.get(&name) else {
                        continue;
                    };
                    let Some(inherited) = capture_names.get(target).cloned() else {
                        continue;
                    };
                    for inherited_name in inherited {
                        if let Some(names) = capture_names.get_mut(index) {
                            changed |= names.insert(inherited_name);
                        }
                    }
                }
                for name in names {
                    let Some(&target) = local_indices.get(name) else {
                        continue;
                    };
                    let Some(inherited) = function_names.get(target).cloned() else {
                        continue;
                    };
                    if let Some(current) = function_names.get_mut(index) {
                        for inherited_name in inherited {
                            changed |= current.insert(inherited_name);
                        }
                    }
                }
            }
        }
        capture_names
            .iter()
            .zip(function_names)
            .map(|(names, functions)| {
                (
                    names
                        .iter()
                        .filter_map(|name| {
                            f.env()
                                .lookup_variable(name)
                                .map(|slot| (name.clone(), slot))
                        })
                        .collect(),
                    functions
                        .into_iter()
                        .filter(|name| {
                            !local_indices.contains_key(name)
                                && f.env().lookup_function(name).is_some()
                        })
                        .collect(),
                )
            })
            .collect()
    }
}
