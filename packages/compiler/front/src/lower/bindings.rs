//! Lexical binding and call-form lowering for the IR v2 path.

use ncl_ir::{HandlerKind, HandlerRegion, HandlerRegionId, OpKind, Terminator, Ty, ValueId};

use crate::ast::Expr;
use crate::symbols::SymbolRef;

use super::super::capture;
use super::super::env::{FunctionEntry, Slot};
use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::super::lambda;
use super::Context;
use super::params::bind_let;

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
                    f.safepoint()?;
                    f.one(
                        OpKind::Builtin {
                            name: "set-symbol-value".to_owned(),
                            args: vec![symbol, value],
                        },
                        Ty::Word,
                    )?;
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
        let mut args = vec![callee];
        for argument in arguments {
            args.push(self.lower_expr(f, argument)?);
        }
        f.safepoint()?;
        f.one(
            OpKind::Builtin {
                name: "multiple-value-call".to_owned(),
                args,
            },
            Ty::Word,
        )
    }

    pub(super) fn lower_local_functions(
        &mut self,
        f: &mut FunctionLowerer,
        definitions: &[crate::ast::LocalFunction],
        body: &[Expr],
    ) -> Result<ValueId, LowerError> {
        f.env().push();
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
}
