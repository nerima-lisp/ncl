//! Lowering every `Expr` variant into `ncl-ir` operations.
//!
//! Every expression yields one `Word` SSA value. Forms whose `ncl-ir`
//! representation is not implemented return [`LowerError::Unsupported`] rather
//! than a value that would misrepresent the program.

use ncl_ir::{Compare, Constant, Convert, OpKind, Terminator, Ty, ValueId};

use crate::ast::{Expr, FunctionDesignator, Operator};

use super::env::{FunctionEntry, Slot};
use super::error::LowerError;
use super::function::{FunctionLowerer, Lowerer};
use super::literal::lower_literal;
use super::{control, lambda as lambda_mod};

/// Lower one expression into a single `Word` value.
///
/// # Errors
///
/// Returns [`LowerError`] when a form has no `ncl-ir` representation.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per frozen AST variant keeps the dispatch auditable"
)]
pub(super) fn lower_expr(
    f: &mut FunctionLowerer,
    module: &mut Lowerer,
    expr: &Expr,
) -> Result<ValueId, LowerError> {
    match expr {
        Expr::Constant(literal) => lower_literal(f, literal),
        Expr::Variable(name) => match f.env().lookup_variable(name) {
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
        },
        Expr::Call {
            operator,
            arguments,
        } => lower_call(f, module, operator, arguments),
        Expr::Function(designator) => lower_designator(f, module, designator),
        Expr::Lambda(lambda) => {
            let captures = lambda_mod::collect_captures(f, lambda);
            lower_lambda_value(f, module, lambda, &captures).map(|(callee, _)| callee)
        }
        Expr::If {
            test,
            then,
            otherwise,
        } => lower_if(f, module, test, then, otherwise.as_deref()),
        Expr::Progn(forms) => lower_body(f, module, forms),
        Expr::Block { name, body } => control::lower_block(f, module, name, body),
        Expr::ReturnFrom { name, value } => {
            control::lower_return_from(f, module, name, value.as_deref())
        }
        Expr::Tagbody(items) => control::lower_tagbody(f, module, items),
        Expr::Go { tag } => control::lower_go(f, tag),
        Expr::Catch { .. } => Err(LowerError::Unsupported { form: "catch" }),
        Expr::Throw { tag, value } => {
            lower_expr(f, module, tag)?;
            let value = lower_expr(f, module, value)?;
            if !f.is_terminated() {
                f.terminate(Terminator::Throw { condition: value })?;
            }
            Ok(value)
        }
        Expr::UnwindProtect { .. } => Err(LowerError::Unsupported {
            form: "unwind-protect",
        }),
        Expr::Let {
            sequential,
            bindings,
            body,
            ..
        } => lower_let(f, module, *sequential, bindings, body),
        Expr::Progv { .. } => Err(LowerError::Unsupported { form: "progv" }),
        Expr::Setq(pairs) => lower_setq(f, module, pairs),
        Expr::MultipleValueCall {
            function,
            arguments,
        } => lower_multiple_value_call(f, module, function, arguments),
        Expr::MultipleValueProg1 { first, forms } => {
            let value = lower_expr(f, module, first)?;
            for form in forms {
                lower_expr(f, module, form)?;
            }
            f.none(OpKind::SetMultipleValues {
                values: vec![value],
            })?;
            Ok(value)
        }
        Expr::The { value: form, .. } | Expr::LoadTimeValue { form, .. } => {
            lower_expr(f, module, form)
        }
        Expr::EvalWhen { body, .. }
        | Expr::Locally { body, .. }
        | Expr::Macrolet { body, .. }
        | Expr::SymbolMacrolet { body, .. } => lower_body(f, module, body),
        Expr::Flet {
            definitions, body, ..
        }
        | Expr::Labels {
            definitions, body, ..
        } => lower_local_functions(f, module, definitions, body),
    }
}

/// Lower a sequence of forms, stopping when a form terminates its block.
///
/// # Errors
///
/// Propagates [`LowerError`] from any form.
pub(super) fn lower_body(
    f: &mut FunctionLowerer,
    module: &mut Lowerer,
    forms: &[Expr],
) -> Result<ValueId, LowerError> {
    let mut value = None;
    for form in forms {
        if f.is_terminated() {
            break;
        }
        value = Some(lower_expr(f, module, form)?);
    }
    value.map_or_else(|| f.nil(), Ok)
}

fn lower_call(
    f: &mut FunctionLowerer,
    module: &mut Lowerer,
    operator: &Operator,
    arguments: &[Expr],
) -> Result<ValueId, LowerError> {
    let evaluated = arguments
        .iter()
        .map(|argument| lower_expr(f, module, argument))
        .collect::<Result<Vec<_>, _>>()?;
    match operator {
        Operator::Name(name) => {
            if let Some(entry) = f.env().lookup_function(name) {
                let (argc, mut captures) = call_prefix(f, &entry.captures, evaluated.len())?;
                let mut all = vec![argc];
                all.append(&mut captures);
                all.extend(evaluated);
                f.safepoint()?;
                return f.one(
                    OpKind::CallIndirect {
                        callee: entry.callee,
                        args: all,
                    },
                    Ty::Word,
                );
            }
            let callee = f.symbol(name)?;
            f.safepoint()?;
            f.one(
                OpKind::Call {
                    function: callee,
                    args: evaluated,
                },
                Ty::Word,
            )
        }
        Operator::Lambda(lambda) => {
            let captures = lambda_mod::collect_captures(f, lambda);
            let (callee, slots) = lower_lambda_value(f, module, lambda, &captures)?;
            let (argc, mut values) = call_prefix(f, &slots, evaluated.len())?;
            let mut all = vec![argc];
            all.append(&mut values);
            all.extend(evaluated);
            f.safepoint()?;
            f.one(OpKind::CallIndirect { callee, args: all }, Ty::Word)
        }
    }
}

/// Build the `argc` argument and the captured arguments a callee expects first.
fn call_prefix(
    f: &mut FunctionLowerer,
    captures: &[Slot],
    actual_count: usize,
) -> Result<(ValueId, Vec<ValueId>), LowerError> {
    let count = i64::try_from(actual_count).map_err(|_| LowerError::Ir {
        detail: "argument count does not fit i64".to_owned(),
    })?;
    let count = f.fixnum(count)?;
    let argc = f.one(
        OpKind::Convert {
            op: Convert::I64ToWord,
            value: count,
        },
        Ty::Word,
    )?;
    let mut values = Vec::new();
    for slot in captures {
        match slot {
            Slot::Value(value) => values.push(*value),
            Slot::Cell(address) => values.push(f.one(
                OpKind::Convert {
                    op: Convert::AddressToWord,
                    value: *address,
                },
                Ty::Word,
            )?),
        }
    }
    Ok((argc, values))
}

/// Lower a lambda to a nested function and a placeholder callee value.
fn lower_lambda_value(
    f: &mut FunctionLowerer,
    module: &mut Lowerer,
    lambda: &crate::ast::LambdaExpr,
    captures: &[lambda_mod::Capture],
) -> Result<(ValueId, Vec<Slot>), LowerError> {
    let name = module.fresh_closure();
    lambda_mod::lower_lambda(module, lambda, captures, name.clone())?;
    let callee = f.word_constant(Constant::Symbol {
        package: "NCL-CLOSURE".to_owned(),
        name,
    })?;
    let slots = captures.iter().map(|(_, slot)| *slot).collect();
    Ok((callee, slots))
}

fn lower_designator(
    f: &mut FunctionLowerer,
    module: &mut Lowerer,
    designator: &FunctionDesignator,
) -> Result<ValueId, LowerError> {
    match designator {
        FunctionDesignator::Name(name) => f.symbol(name),
        FunctionDesignator::Lambda(lambda) => {
            let captures = lambda_mod::collect_captures(f, lambda);
            lower_lambda_value(f, module, lambda, &captures).map(|(callee, _)| callee)
        }
    }
}

fn lower_if(
    f: &mut FunctionLowerer,
    module: &mut Lowerer,
    test: &Expr,
    then: &Expr,
    otherwise: Option<&Expr>,
) -> Result<ValueId, LowerError> {
    let tested = lower_expr(f, module, test)?;
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
    let then_block = f.create_block(Vec::new());
    let else_block = f.create_block(Vec::new());
    let merge = f.create_block(vec![(Ty::Word, result)]);
    f.position(start)?;
    f.terminate(Terminator::Branch {
        condition,
        then_target: then_block,
        then_args: Vec::new(),
        else_target: else_block,
        else_args: Vec::new(),
    })?;
    f.position(then_block)?;
    let then_value = lower_expr(f, module, then)?;
    if !f.is_terminated() {
        f.terminate(Terminator::Jump {
            target: merge,
            args: vec![then_value],
        })?;
    }
    f.position(else_block)?;
    let else_value = match otherwise {
        Some(form) => lower_expr(f, module, form)?,
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

fn lower_let(
    f: &mut FunctionLowerer,
    module: &mut Lowerer,
    sequential: bool,
    bindings: &[crate::ast::LetBinding],
    body: &[Expr],
) -> Result<ValueId, LowerError> {
    let analysis = super::capture::analyze(body);
    f.env().push();
    if sequential {
        for binding in bindings {
            let value = match &binding.value {
                Some(form) => lower_expr(f, module, form)?,
                None => f.nil()?,
            };
            bind_let(f, &binding.name, value, &analysis)?;
        }
    } else {
        let mut evaluated = Vec::new();
        for binding in bindings {
            let value = match &binding.value {
                Some(form) => lower_expr(f, module, form)?,
                None => f.nil()?,
            };
            evaluated.push((binding.name.clone(), value));
        }
        for (name, value) in evaluated {
            bind_let(f, &name, value, &analysis)?;
        }
    }
    let value = lower_body(f, module, body)?;
    f.env().pop();
    Ok(value)
}

/// Bind a `let` variable, boxing it into a one-word cell when the body assigns
/// and captures it.
fn bind_let(
    f: &mut FunctionLowerer,
    name: &crate::symbols::SymbolRef,
    value: ValueId,
    analysis: &super::capture::Analysis,
) -> Result<(), LowerError> {
    if analysis.needs_cell(name) {
        let address = f.one(OpKind::Alloc { words: 1 }, Ty::Address)?;
        f.safepoint()?;
        f.none(OpKind::Store { address, value })?;
        f.env().bind_variable(name.clone(), Slot::Cell(address));
    } else {
        f.env().bind_variable(name.clone(), Slot::Value(value));
    }
    Ok(())
}

fn lower_setq(
    f: &mut FunctionLowerer,
    module: &mut Lowerer,
    pairs: &[(crate::symbols::SymbolRef, Expr)],
) -> Result<ValueId, LowerError> {
    let mut last = None;
    for (name, form) in pairs {
        let value = lower_expr(f, module, form)?;
        match f.env().lookup_variable(name) {
            Some(Slot::Cell(address)) => {
                f.none(OpKind::Store { address, value })?;
            }
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

fn lower_multiple_value_call(
    f: &mut FunctionLowerer,
    module: &mut Lowerer,
    function: &Expr,
    arguments: &[Expr],
) -> Result<ValueId, LowerError> {
    let callee = lower_expr(f, module, function)?;
    let mut args = vec![callee];
    for argument in arguments {
        args.push(lower_expr(f, module, argument)?);
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

fn lower_local_functions(
    f: &mut FunctionLowerer,
    module: &mut Lowerer,
    definitions: &[crate::ast::LocalFunction],
    body: &[Expr],
) -> Result<ValueId, LowerError> {
    f.env().push();
    for definition in definitions {
        let captures = lambda_mod::collect_captures(f, &definition.lambda);
        let name = module.fresh_closure();
        lambda_mod::lower_lambda(module, &definition.lambda, &captures, name.clone())?;
        let callee = f.word_constant(Constant::Symbol {
            package: "NCL-CLOSURE".to_owned(),
            name,
        })?;
        f.env().bind_function(FunctionEntry {
            name: definition.name.clone(),
            callee,
            captures: captures.into_iter().map(|(_, slot)| slot).collect(),
        });
    }
    let value = lower_body(f, module, body)?;
    f.env().pop();
    Ok(value)
}
