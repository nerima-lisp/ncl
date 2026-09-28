//! Lexical binding and call-form lowering for the IR v2 path.

use ncl_ir::{
    Constant, Convert, FunctionId, HandlerKind, HandlerRegion, HandlerRegionId, OpKind, Terminator,
    Ty, ValueId,
};

use crate::ast::Expr;
use crate::symbols::SymbolRef;

use super::super::capture;
use super::super::env::{FunctionEntry, Slot};
use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::super::lambda;
use super::Context;
use super::params::{bind_captures, bind_let, bind_required, lambda_params};

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
        body: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let analysis = capture::analyze(body);
        f.env().push();
        if sequential {
            for binding in bindings {
                let value = match binding.value.as_ref() {
                    Some(form) => self.lower_expr(f, form)?,
                    None => f.nil()?,
                };
                bind_let(f, &binding.name, value, &analysis)?;
            }
        } else {
            let mut evaluated = Vec::new();
            for binding in bindings {
                let value = match binding.value.as_ref() {
                    Some(form) => self.lower_expr(f, form)?,
                    None => f.nil()?,
                };
                evaluated.push((binding.name.clone(), value));
            }
            for (name, value) in evaluated {
                bind_let(f, &name, value, &analysis)?;
            }
        }
        let value = self.lower_body(f, body)?;
        f.env().pop();
        Ok(value)
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
                Some(Slot::Cell(address)) => f.none(OpKind::Store { address, value })?,
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
            let captures = lambda::collect_captures(f, &definition.lambda);
            let closure = self.lower_lambda_value(f, &definition.lambda)?;
            let _ = captures;
            f.env().bind_function(FunctionEntry {
                name: definition.name.clone(),
                callee: closure,
            });
        }
        let value = self.lower_body(f, body)?;
        f.env().pop();
        Ok(value)
    }

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
        for (definition, id) in definitions.iter().zip(ids.iter().copied()) {
            let captures = lambda::collect_captures(f, &definition.lambda);
            let params = lambda_params(&definition.lambda.lambda_list, &captures)?;
            let mut nested =
                FunctionLowerer::new(id, format!("lambda-{id:?}"), params, vec![Ty::Word]);
            bind_captures(&mut nested, &captures)?;
            bind_required(
                &mut nested,
                &definition.lambda.lambda_list,
                1 + captures.len(),
            )?;
            let mut child = Context::with_targets(self.module, self.targets.clone());
            child.bind_optional(
                &mut nested,
                &definition.lambda.lambda_list,
                1 + captures.len(),
            )?;
            child.bind_rest_and_keys(&mut nested, &definition.lambda.lambda_list)?;
            child.bind_aux(&mut nested, &definition.lambda.lambda_list)?;
            for (target, target_id) in definitions.iter().zip(ids.iter().copied()) {
                let target_captures = lambda::collect_captures(f, &target.lambda);
                let capture_values = target_captures
                    .iter()
                    .map(|(name, _slot)| match nested.env().lookup_variable(name) {
                        Some(Slot::Value(value)) => Ok(value),
                        Some(Slot::Cell(address)) => nested.one(
                            OpKind::Convert {
                                op: Convert::AddressToWord,
                                value: address,
                            },
                            Ty::Word,
                        ),
                        None => Err(LowerError::Ir {
                            detail: format!("recursive function capture is unavailable: {name}"),
                        }),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
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
            let capture_values = captures
                .iter()
                .map(|(_, slot)| match slot {
                    Slot::Value(value) => Ok(*value),
                    Slot::Cell(address) => f.one(
                        OpKind::Convert {
                            op: Convert::AddressToWord,
                            value: *address,
                        },
                        Ty::Word,
                    ),
                })
                .collect::<Result<Vec<_>, LowerError>>()?;
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
}
