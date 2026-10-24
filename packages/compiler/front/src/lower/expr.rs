//! Expression lowering for the IR v2 path.

#[path = "expr/symbol_cell.rs"]
mod symbol_cell;

use ncl_ir::{Constant, Convert, FunctionId, OpKind, Ty, ValueId};

use crate::ast::{EvalSituation, Expr, FunctionDesignator, LambdaExpr, Operator};
use crate::symbols::SymbolRef;

use super::super::env::FunctionEntry;
use super::super::env::Slot;
use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::super::lambda;
use super::super::literal::lower_literal;
use super::Context;
use super::analysis::mentions_exit;
use super::params::{bind_captures, lambda_params};

fn is_closure_designator(expr: &Expr) -> bool {
    match expr {
        Expr::Lambda(_) | Expr::Function(FunctionDesignator::Lambda(_)) => true,
        Expr::Let { body, .. } | Expr::Progn(body) => {
            body.last().is_some_and(is_closure_designator)
        }
        Expr::Locally { body, .. }
        | Expr::Macrolet { body, .. }
        | Expr::SymbolMacrolet { body, .. } => body.last().is_some_and(is_closure_designator),
        Expr::The { value, .. } | Expr::LoadTimeValue { form: value, .. } => {
            is_closure_designator(value)
        }
        // check-added-lines: allow(wildcard) other forms do not prove a closure result.
        _ => false,
    }
}

impl Context<'_> {
    pub(crate) fn lower_expr(
        &mut self,
        f: &mut FunctionLowerer,
        expr: &Expr,
    ) -> Result<ValueId, LowerError> {
        if self.preserve_multiple_values {
            let nil = f.nil()?;
            f.none(OpKind::SetMultipleValues { values: vec![nil] })?;
        }
        match expr {
            Expr::Constant(literal) => lower_literal(f, literal),
            Expr::Variable(name) => {
                let value = self.lower_variable(f, name)?;
                if f.env().lookup_variable(name).is_some()
                    && let Some(ty) = self.variable_hint(name)
                {
                    Self::type_hint(f, value, ty)
                } else {
                    Ok(value)
                }
            }
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
                declarations,
                body,
            } => self.lower_let(f, *sequential, bindings, declarations, body),
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
                self.lower_multiple_value_prog1(f, first, forms)
            }
            Expr::The {
                type_specifier,
                value,
            } => {
                let value = self.lower_expr(f, value)?;
                super::scalar_hint(type_specifier.form())
                    .map_or(Ok(value), |ty| Self::type_hint(f, value, ty))
            }
            Expr::LoadTimeValue { form: value, .. } => self.lower_expr(f, value),
            Expr::EvalWhen { situations, body } => {
                if situations.iter().any(|situation| {
                    matches!(situation, EvalSituation::Execute | EvalSituation::Eval)
                }) {
                    self.lower_body(f, body)
                } else {
                    f.word_constant(Constant::Nil)
                }
            }
            Expr::Locally { declarations, body }
            | Expr::Macrolet {
                declarations, body, ..
            }
            | Expr::SymbolMacrolet {
                declarations, body, ..
            } => self.lower_declared_body(f, declarations, body, None),
            Expr::Flet {
                definitions,
                declarations,
                body,
            } => self.lower_declared_body(f, declarations, body, Some((definitions, false))),
            Expr::Labels {
                definitions,
                declarations,
                body,
            } => self.lower_declared_body(f, declarations, body, Some((definitions, true))),
        }
    }

    fn lower_declared_body(
        &mut self,
        f: &mut FunctionLowerer,
        declarations: &[crate::declaration::Declaration],
        body: &[Expr],
        local_functions: Option<(&[crate::ast::LocalFunction], bool)>,
    ) -> Result<ValueId, LowerError> {
        self.push_declarations(declarations);
        let result = if let Some((definitions, recursive)) = local_functions {
            self.lower_local_functions(f, definitions, body, recursive)
        } else {
            self.lower_body(f, body)
        };
        self.pop_declarations();
        result
    }

    fn type_hint(
        f: &mut FunctionLowerer,
        value: ValueId,
        ty: ncl_ir::ScalarTypeHint,
    ) -> Result<ValueId, LowerError> {
        f.one(OpKind::TypeHint { value, ty }, Ty::Word)
    }

    fn lower_multiple_value_prog1(
        &mut self,
        f: &mut FunctionLowerer,
        first: &Expr,
        forms: &[Expr],
    ) -> Result<ValueId, LowerError> {
        let value = self.lower_expr(f, first)?;
        if self.preserve_multiple_values {
            let saved_values = Self::capture_multiple_values(f, value)?;
            for form in forms {
                self.lower_expr(f, form)?;
            }
            Self::restore_multiple_values(f, saved_values)
        } else {
            for form in forms {
                self.lower_expr(f, form)?;
            }
            f.none(OpKind::SetMultipleValues {
                values: vec![value],
            })?;
            Ok(value)
        }
    }

    fn lower_variable(
        &mut self,
        f: &mut FunctionLowerer,
        name: &SymbolRef,
    ) -> Result<ValueId, LowerError> {
        match f.env().lookup_variable(name) {
            Some(Slot::Value(value)) => Ok(value),
            Some(Slot::Cell(cell)) => f.one(
                OpKind::LoadField {
                    object: cell,
                    field: 0,
                },
                Ty::Word,
            ),
            None => {
                let symbol = f.symbol(name)?;
                self.load_bound_symbol_value(f, symbol)
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn lower_call(
        &mut self,
        f: &mut FunctionLowerer,
        operator: &Operator,
        arguments: &[Expr],
    ) -> Result<ValueId, LowerError> {
        // Stop lowering arguments once a prior one terminates the current
        // block (e.g. `(return-from b 7)`/`(go tag)`/`(throw 'k 5)` used as a
        // non-tail argument): later arguments would otherwise emit ops into
        // an already-terminated block.
        let mut values = Vec::with_capacity(arguments.len());
        for argument in arguments {
            if f.is_terminated() {
                break;
            }
            values.push(self.lower_expr(f, argument)?);
        }
        if f.is_terminated() {
            // A non-tail argument transferred control away for good
            // (`return-from`/`go`/`throw`); the call itself is unreachable,
            // so there is nothing left to lower. The result is never
            // observed by whatever terminator already ran.
            return f.nil();
        }
        if let Operator::Name(name) = operator
            && name.name.eq_ignore_ascii_case("FUNCALL")
            && arguments.first().is_some_and(is_closure_designator)
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
                    named_symbol: None,
                },
                Ty::Word,
            );
        }
        if let Operator::Name(name) = operator
            && let Some(result) = self.lower_symbol_cell_call(f, name, &values)?
        {
            return self.apply_function_hint(f, operator, result);
        }
        if let Operator::Name(name) = operator
            && ((matches!(name.name.as_str(), "+" | "*" | "-" | "<") && values.len() == 2)
                || (values.len() == 1
                    && (name.is_common_lisp("1+") || name.is_common_lisp("1-"))
                    && f.env().lookup_function(name).is_none())
                || (name.name == "CAR" && values.len() == 1)
                || (name.name == "CONS" && values.len() == 2))
        {
            f.safepoint()?;
            let result = f.one(
                OpKind::Builtin {
                    name: name.name.clone(),
                    args: values,
                    specialization: None,
                    specialization_disabled: self.specialization_disabled(),
                },
                Ty::Word,
            )?;
            return self.apply_function_hint(f, operator, result);
        }
        let (callee, closure, named_symbol) = match operator {
            Operator::Name(name) => {
                if let Some(entry) = f.env().lookup_function(name) {
                    (entry.callee, true, None)
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
                    (function, true, Some(symbol))
                }
            }
            Operator::Lambda(lambda) => (self.lower_lambda_value(f, lambda)?, true, None),
        };
        let argc_value = Self::argc(f, values.len())?;
        let mut args = vec![argc_value];
        args.extend(values);
        f.safepoint()?;
        let result = if closure {
            f.one(
                OpKind::CallClosure {
                    closure: callee,
                    args,
                    named_symbol,
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
        }?;
        self.apply_function_hint(f, operator, result)
    }

    fn apply_function_hint(
        &self,
        f: &mut FunctionLowerer,
        operator: &Operator,
        value: ValueId,
    ) -> Result<ValueId, LowerError> {
        let Operator::Name(name) = operator else {
            return Ok(value);
        };
        self.function_hint(name)
            .map_or(Ok(value), |ty| Self::type_hint(f, value, ty))
    }

    pub(super) fn argc(f: &mut FunctionLowerer, count: usize) -> Result<ValueId, LowerError> {
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
            FunctionDesignator::Name(name) => {
                if let Some(entry) = f.env().lookup_function(name) {
                    return Ok(entry.callee);
                }
                let symbol = f.symbol(name)?;
                f.one(
                    OpKind::LoadField {
                        object: symbol,
                        field: u32::try_from(ncl_object::symbol_offset::FUNCTION).map_err(
                            |_| LowerError::Ir {
                                detail: "function symbol offset does not fit u32".to_owned(),
                            },
                        )?,
                    },
                    Ty::Word,
                )
            }
            FunctionDesignator::Lambda(lambda) => self.lower_lambda_value(f, lambda),
        }
    }

    pub(super) fn lower_lambda_value(
        &mut self,
        f: &mut FunctionLowerer,
        lambda: &LambdaExpr,
    ) -> Result<ValueId, LowerError> {
        self.lower_lambda_value_with(f, lambda, &[], None)
    }

    pub(super) fn lower_lambda_value_with(
        &mut self,
        f: &mut FunctionLowerer,
        lambda: &LambdaExpr,
        extra_captures: &[(SymbolRef, Slot)],
        self_name: Option<&SymbolRef>,
    ) -> Result<ValueId, LowerError> {
        let mut captures = lambda::collect_captures(f, lambda);
        for (name, slot) in extra_captures {
            if !captures.iter().any(|(capture, _)| capture == name) {
                captures.push((name.clone(), *slot));
            }
        }
        let function_captures = lambda::collect_function_captures(f, lambda);
        for target in &self.targets {
            if mentions_exit(&lambda.body, &target.name)
                && !captures.iter().any(|(name, _)| name == &target.capture)
                && let Some(slot) = f.env().lookup_variable(&target.capture)
            {
                captures.push((target.capture.clone(), slot));
            }
        }
        let id = self.lower_lambda(lambda, &captures, &function_captures, self_name)?;
        let entry = f.word_constant(Constant::FunctionEntry(id))?;
        let capture_values = captures
            .iter()
            .map(|(_, slot)| match slot {
                Slot::Value(value) => Ok(*value),
                Slot::Cell(cell) => Ok(*cell),
            })
            .collect::<Result<Vec<_>, LowerError>>()?;
        let mut capture_values = capture_values;
        capture_values.extend(function_captures.iter().map(|(_, value)| *value));
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
        function_captures: &[super::super::lambda::FunctionCapture],
        self_name: Option<&SymbolRef>,
    ) -> Result<FunctionId, LowerError> {
        let id = self.module.fresh_function();
        let params = lambda_params(&lambda.lambda_list)?;
        let mut nested = FunctionLowerer::new(id, format!("lambda-{id:?}"), params, vec![Ty::Word]);
        bind_captures(&mut nested, captures)?;
        for (index, (name, _)) in function_captures.iter().enumerate() {
            let value = nested.one(
                OpKind::LoadCapture {
                    index: u8::try_from(captures.len() + index).map_err(|_| LowerError::Ir {
                        detail: "function capture index does not fit u8".to_owned(),
                    })?,
                },
                Ty::Word,
            )?;
            nested.env().bind_function(FunctionEntry {
                name: name.clone(),
                callee: value,
            });
        }
        let inherited_facts = self.lexical_facts.clone();
        let mut child = Context::with_targets(
            self.module,
            self.targets.clone(),
            self.preserve_multiple_values,
        );
        child.lexical_facts = inherited_facts;
        child.bind_required(&mut nested, &lambda.lambda_list, 1)?;
        if let Some(name) = self_name {
            let callee = nested.one(OpKind::LoadFunctionObject, Ty::Word)?;
            nested.env().bind_function(FunctionEntry {
                name: name.clone(),
                callee,
            });
        }
        child.bind_optional(&mut nested, &lambda.lambda_list, 1)?;
        child.bind_rest_and_keys(&mut nested, &lambda.lambda_list)?;
        child.bind_aux(&mut nested, &lambda.lambda_list)?;
        child.push_declarations(&lambda.declarations);
        let value = child.lower_body(&mut nested, &lambda.body)?;
        child.pop_declarations();
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
}
