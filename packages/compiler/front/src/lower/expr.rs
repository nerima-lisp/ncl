//! Expression lowering for the IR v2 path.

use ncl_ir::{Compare, Constant, Convert, FunctionId, OpKind, Terminator, Ty, ValueId};

use crate::ast::{Expr, FunctionDesignator, LambdaExpr, Operator};
use crate::symbols::SymbolRef;

use super::super::env::Slot;
use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::super::lambda;
use super::super::literal::lower_literal;
use super::Context;
use super::params::{bind_captures, bind_required, lambda_params};

impl Context<'_> {
    pub(crate) fn lower_expr(
        &mut self,
        f: &mut FunctionLowerer,
        expr: &Expr,
    ) -> Result<ValueId, LowerError> {
        match expr {
            Expr::Constant(literal) => lower_literal(f, literal),
            Expr::Variable(name) => Self::lower_variable(f, name),
            Expr::Call {
                operator,
                arguments,
            } => self.lower_call(f, operator, arguments),
            Expr::Function(designator) => self.lower_designator(f, designator),
            Expr::Lambda(lambda) => self.lower_lambda_value(f, lambda),
            Expr::If {
                test,
                then,
                otherwise,
            } => self.lower_if(f, test, then, otherwise.as_deref()),
            Expr::Progn(forms) => self.lower_body(f, forms),
            Expr::Block { name, body } => self.lower_block(f, name, body),
            Expr::ReturnFrom { name, value } => self.lower_return_from(f, name, value.as_deref()),
            Expr::Tagbody(items) => self.lower_tagbody(f, items),
            Expr::Go { tag } => self.lower_go(f, tag),
            Expr::Catch { tag, body } => self.lower_catch(f, tag, body),
            Expr::Throw { tag, value } => self.lower_throw(f, tag, value),
            Expr::UnwindProtect { protected, cleanup } => self.lower_unwind(f, protected, cleanup),
            Expr::Let {
                sequential,
                bindings,
                body,
                ..
            } => self.lower_let(f, *sequential, bindings, body),
            Expr::Progv {
                symbols,
                values,
                body,
            } => self.lower_progv(f, symbols, values, body),
            Expr::Setq(pairs) => self.lower_setq(f, pairs),
            Expr::MultipleValueCall {
                function,
                arguments,
            } => self.lower_multiple_value_call(f, function, arguments),
            Expr::MultipleValueProg1 { first, forms } => {
                let value = self.lower_expr(f, first)?;
                for form in forms {
                    self.lower_expr(f, form)?;
                }
                f.none(OpKind::SetMultipleValues {
                    values: vec![value],
                })?;
                Ok(value)
            }
            Expr::The { value, .. } | Expr::LoadTimeValue { form: value, .. } => {
                self.lower_expr(f, value)
            }
            Expr::EvalWhen { body, .. }
            | Expr::Locally { body, .. }
            | Expr::Macrolet { body, .. }
            | Expr::SymbolMacrolet { body, .. } => self.lower_body(f, body),
            Expr::Flet {
                definitions, body, ..
            }
            | Expr::Labels {
                definitions, body, ..
            } => self.lower_local_functions(f, definitions, body),
        }
    }

    fn lower_variable(f: &mut FunctionLowerer, name: &SymbolRef) -> Result<ValueId, LowerError> {
        match f.env().lookup_variable(name) {
            Some(Slot::Value(value)) => Ok(value),
            Some(Slot::Cell(address)) => f.one(OpKind::Load { address }, Ty::Word),
            None => {
                let symbol = f.symbol(name)?;
                f.one(
                    OpKind::LoadField {
                        object: symbol,
                        field: u32::try_from(ncl_object::symbol_offset::VALUE).map_err(|_| {
                            LowerError::Ir {
                                detail: "symbol value offset does not fit u32".to_owned(),
                            }
                        })?,
                    },
                    Ty::Bool,
                )
            }
        }
    }

    fn lower_call(
        &mut self,
        f: &mut FunctionLowerer,
        operator: &Operator,
        arguments: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let values = arguments
            .iter()
            .map(|argument| self.lower_expr(f, argument))
            .collect::<Result<Vec<_>, _>>()?;
        if let Operator::Name(name) = operator
            && name.name.eq_ignore_ascii_case("FUNCALL")
        {
            let Some((&callee, call_arguments)) = values.split_first() else {
                return Err(LowerError::Ir {
                    detail: "funcall requires a function designator".to_owned(),
                });
            };
            let argc = Self::argc(f, call_arguments.len())?;
            let mut call_args = vec![argc];
            call_args.extend_from_slice(call_arguments);
            f.safepoint()?;
            return f.one(
                OpKind::CallClosure {
                    closure: callee,
                    args: call_args,
                },
                Ty::Word,
            );
        }
        if let Operator::Name(name) = operator
            && let Some(result) = Self::lower_symbol_cell_call(f, name, &values)?
        {
            return Ok(result);
        }
        if let Operator::Name(name) = operator
            && ((matches!(name.name.as_str(), "+" | "*" | "-" | "<") && values.len() == 2)
                || (name.name == "CAR" && values.len() == 1)
                || (name.name == "CONS" && values.len() == 2))
        {
            f.safepoint()?;
            return f.one(
                OpKind::Builtin {
                    name: name.name.clone(),
                    args: values,
                },
                Ty::Word,
            );
        }
        let (callee, closure) = match operator {
            Operator::Name(name) => {
                if let Some(entry) = f.env().lookup_function(name) {
                    (entry.callee, true)
                } else {
                    let symbol = f.symbol(name)?;
                    let function = f.one(
                        OpKind::LoadField {
                            object: symbol,
                            field: u32::try_from(ncl_object::symbol_offset::FUNCTION).map_err(
                                |_| LowerError::Ir {
                                    detail: "function symbol offset does not fit u32".to_owned(),
                                },
                            )?,
                        },
                        Ty::Word,
                    )?;
                    (function, true)
                }
            }
            Operator::Lambda(lambda) => (self.lower_lambda_value(f, lambda)?, true),
        };
        let argc_value = Self::argc(f, values.len())?;
        let mut args = vec![argc_value];
        args.extend(values);
        f.safepoint()?;
        if closure {
            f.one(
                OpKind::CallClosure {
                    closure: callee,
                    args,
                },
                Ty::Word,
            )
        } else {
            f.one(
                OpKind::Call {
                    function: callee,
                    args,
                },
                Ty::Word,
            )
        }
    }

    /// Lower a call to `symbol-value`, `set-symbol-value`, or
    /// `fdefinition-set` into a direct symbol-cell load or store.
    ///
    /// Returns `Ok(None)` when `name`/`values` do not match one of those
    /// three forms, so the caller falls through to ordinary call lowering.
    fn lower_symbol_cell_call(
        f: &mut FunctionLowerer,
        name: &SymbolRef,
        values: &[ValueId],
    ) -> Result<Option<ValueId>, LowerError> {
        let field =
            if matches!(name.name.as_str(), "symbol-value" | "SYMBOL-VALUE") && values.len() == 1 {
                Some((ncl_object::symbol_offset::VALUE, false))
            } else if name.name == "set-symbol-value" && values.len() == 2 {
                Some((ncl_object::symbol_offset::VALUE, true))
            } else if (name.is_named("COMMON-LISP", "NCL::FDEFINITION-SET")
                || name.is_named("NCL", "FDEFINITION-SET")
                || name.is_named("NCL-EXT", "FDEFINITION-SET"))
                && values.len() == 2
            {
                Some((ncl_object::symbol_offset::FUNCTION, true))
            } else {
                None
            };
        let Some((field, store)) = field else {
            return Ok(None);
        };
        let field = u32::try_from(field).map_err(|_| LowerError::Ir {
            detail: "symbol cell offset does not fit u32".to_owned(),
        })?;
        let object = values.first().copied().ok_or_else(|| LowerError::Ir {
            detail: "symbol cell operation requires a symbol".to_owned(),
        })?;
        if store {
            let value = values.get(1).copied().ok_or_else(|| LowerError::Ir {
                detail: "symbol cell store requires a value".to_owned(),
            })?;
            f.none(OpKind::StoreField {
                object,
                field,
                value,
            })?;
            return Ok(Some(value));
        }
        f.one(OpKind::LoadField { object, field }, Ty::Word)
            .map(Some)
    }

    fn argc(f: &mut FunctionLowerer, count: usize) -> Result<ValueId, LowerError> {
        let raw = f.fixnum(i64::try_from(count).map_err(|_| LowerError::Ir {
            detail: "argument count does not fit i64".to_owned(),
        })?)?;
        f.one(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: raw,
            },
            Ty::Word,
        )
    }

    fn lower_designator(
        &mut self,
        f: &mut FunctionLowerer,
        designator: &FunctionDesignator,
    ) -> Result<ValueId, LowerError> {
        match designator {
            FunctionDesignator::Name(name) => f.symbol(name),
            FunctionDesignator::Lambda(lambda) => self.lower_lambda_value(f, lambda),
        }
    }

    pub(super) fn lower_lambda_value(
        &mut self,
        f: &mut FunctionLowerer,
        lambda: &LambdaExpr,
    ) -> Result<ValueId, LowerError> {
        let captures = lambda::collect_captures(f, lambda);
        let id = self.lower_lambda(lambda, &captures)?;
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
        f.one(
            OpKind::MakeClosure {
                entry,
                captures: capture_values,
            },
            Ty::Word,
        )
    }

    fn lower_lambda(
        &mut self,
        lambda: &LambdaExpr,
        captures: &[super::super::lambda::Capture],
    ) -> Result<FunctionId, LowerError> {
        let id = self.module.fresh_function();
        let params = lambda_params(&lambda.lambda_list, captures)?;
        let mut nested = FunctionLowerer::new(id, format!("lambda-{id:?}"), params, vec![Ty::Word]);
        bind_captures(&mut nested, captures)?;
        bind_required(&mut nested, &lambda.lambda_list, 1 + captures.len())?;
        let mut child = Context::with_targets(self.module, self.targets.clone());
        child.bind_optional(&mut nested, &lambda.lambda_list, 1 + captures.len())?;
        child.bind_rest_and_keys(&mut nested, &lambda.lambda_list)?;
        child.bind_aux(&mut nested, &lambda.lambda_list)?;
        let value = child.lower_body(&mut nested, &lambda.body)?;
        if !nested.is_terminated() {
            nested.return_value(value)?;
        }
        let mut function = nested.into_function();
        function.handler_regions = child.regions;
        self.module.push_function(function);
        Ok(id)
    }

    pub(super) fn lower_body(
        &mut self,
        f: &mut FunctionLowerer,
        forms: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let mut value = None;
        for form in forms {
            if f.is_terminated() {
                break;
            }
            value = Some(self.lower_expr(f, form)?);
        }
        value.map_or_else(|| f.nil(), Ok)
    }

    fn lower_if(
        &mut self,
        f: &mut FunctionLowerer,
        test: &Expr,
        then: &Expr,
        otherwise: Option<&Expr>,
    ) -> Result<ValueId, LowerError> {
        let tested = self.lower_expr(f, test)?;
        let nil = f.nil()?;
        let condition = f.one(
            OpKind::Compare {
                op: Compare::Ne,
                left: tested,
                right: nil,
            },
            Ty::Bool,
        )?;
        let start = f.current_block();
        let result = f.fresh_value();
        let then_block = self.block(f, Vec::new());
        let else_block = self.block(f, Vec::new());
        let merge = self.block(f, vec![(Ty::Word, result)]);
        f.position(start)?;
        f.terminate(Terminator::Branch {
            condition,
            then_target: then_block,
            then_args: Vec::new(),
            else_target: else_block,
            else_args: Vec::new(),
        })?;
        f.position(then_block)?;
        let then_value = self.lower_expr(f, then)?;
        if !f.is_terminated() {
            f.terminate(Terminator::Jump {
                target: merge,
                args: vec![then_value],
            })?;
        }
        f.position(else_block)?;
        let else_value = match otherwise {
            Some(form) => self.lower_expr(f, form)?,
            None => f.nil()?,
        };
        if !f.is_terminated() {
            f.terminate(Terminator::Jump {
                target: merge,
                args: vec![else_value],
            })?;
        }
        f.position(merge)?;
        Ok(result)
    }
}
