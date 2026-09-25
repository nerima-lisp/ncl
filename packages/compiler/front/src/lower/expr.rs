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
use super::params::{bind_captures, bind_required, lambda_params, reject_lambda_list};

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
                f.safepoint()?;
                f.one(
                    OpKind::Builtin {
                        name: "symbol-value".to_owned(),
                        args: vec![symbol],
                    },
                    Ty::Word,
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
        let (callee, closure) = match operator {
            Operator::Name(name) => match f.env().lookup_function(name) {
                Some(entry) => (entry.callee, true),
                None => (f.symbol(name)?, false),
            },
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
        reject_lambda_list(&lambda.lambda_list)?;
        let id = self.module.fresh_function();
        let params = lambda_params(&lambda.lambda_list, captures)?;
        let mut nested = FunctionLowerer::new(id, format!("lambda-{id:?}"), params, vec![Ty::Word]);
        bind_captures(&mut nested, captures)?;
        bind_required(&mut nested, &lambda.lambda_list, 1 + captures.len())?;
        let mut child = Context::with_targets(self.module, self.targets.clone());
        child.bind_optional(&mut nested, &lambda.lambda_list, 1 + captures.len())?;
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
