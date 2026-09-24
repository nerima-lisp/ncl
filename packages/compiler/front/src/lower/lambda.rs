//! Lowering a lambda expression into its own `ncl-ir` function.

use ncl_ir::{Compare, Convert, FunctionId, OpKind, Param, Terminator, Ty};

use crate::ast::LambdaExpr;
use crate::lambda_list::{LambdaList, ParamName};
use crate::symbols::SymbolRef;

use super::env::Slot;
use super::error::LowerError;
use super::expr::lower_expr;
use super::function::{FunctionLowerer, Lowerer};

/// The `argc` parameter every generated lambda takes first.
const ARGC: Ty = Ty::Word;

/// A captured variable and how its value is obtained in the enclosing function.
pub(super) type Capture = (SymbolRef, Slot);

/// The free variables of `lambda` that are bound in the enclosing function.
///
/// A free variable with no enclosing lexical binding is special or global; it is
/// not captured and resolves through the symbol's value cell inside the lambda.
pub(super) fn collect_captures(f: &mut FunctionLowerer, lambda: &LambdaExpr) -> Vec<Capture> {
    let mut captures = Vec::new();
    for name in super::capture::free_variables(lambda) {
        if let Some(slot) = f.env().lookup_variable(&name) {
            captures.push((name, slot));
        }
    }
    captures
}

/// Lower a lambda into a new function and return its identity.
///
/// # Errors
///
/// Returns [`LowerError::UnsupportedLambdaList`] for `&rest` and `&key`, whose
/// prologue the code generator owns, and for destructuring parameters, which an
/// ordinary lambda list never contains.
pub(super) fn lower_lambda(
    module: &mut Lowerer,
    lambda: &LambdaExpr,
    captures: &[Capture],
    name: String,
) -> Result<FunctionId, LowerError> {
    let list = &lambda.lambda_list;
    reject_unsupported(list)?;
    let params = build_params(list, captures)?;
    let id = module.fresh_function();
    let mut f = FunctionLowerer::new(id, name, params, vec![Ty::Word]);
    bind_captures(&mut f, captures)?;
    let base = 1 + captures.len();
    bind_required(&mut f, list, base)?;
    bind_optional(&mut f, module, list, base)?;
    bind_aux(&mut f, module, list)?;
    let value = super::expr::lower_body(&mut f, module, &lambda.body)?;
    f.return_value(value)?;
    module.push_function(f.into_function());
    Ok(id)
}

/// Reject lambda list features the lowering lane does not implement.
const fn reject_unsupported(list: &LambdaList) -> Result<(), LowerError> {
    if list.rest.is_some() {
        return Err(LowerError::UnsupportedLambdaList { feature: "&rest" });
    }
    if !list.keys.is_empty() || list.allow_other_keys {
        return Err(LowerError::UnsupportedLambdaList { feature: "&key" });
    }
    if list.whole.is_some() || list.environment.is_some() || list.body.is_some() {
        return Err(LowerError::UnsupportedLambdaList {
            feature: "macro lambda list keyword",
        });
    }
    Ok(())
}

/// The parameter name of an ordinary lambda list element.
fn param_name(name: &ParamName) -> Result<SymbolRef, LowerError> {
    match name {
        ParamName::Symbol(symbol) => Ok(symbol.clone()),
        ParamName::Pattern(_) => Err(LowerError::UnsupportedLambdaList {
            feature: "destructuring parameter",
        }),
    }
}

/// Build the function's parameter list: `argc`, captures, then the declared ones.
fn build_params(list: &LambdaList, captures: &[Capture]) -> Result<Vec<Param>, LowerError> {
    let mut params = vec![Param {
        name: "argc".to_owned(),
        ty: ARGC,
    }];
    for (name, _) in captures {
        params.push(Param {
            name: name.name.clone(),
            ty: Ty::Word,
        });
    }
    for name in &list.required {
        params.push(Param {
            name: param_name(name)?.name,
            ty: Ty::Word,
        });
    }
    for optional in &list.optional {
        params.push(Param {
            name: param_name(&optional.name)?.name,
            ty: Ty::Word,
        });
    }
    Ok(params)
}

/// Load each captured parameter and bind it in the new function's environment.
fn bind_captures(f: &mut FunctionLowerer, captures: &[Capture]) -> Result<(), LowerError> {
    for (index, (name, slot)) in captures.iter().enumerate() {
        let parameter = param_index(1 + index)?;
        let loaded = f.one(OpKind::LoadArg { index: parameter }, Ty::Word)?;
        match slot {
            Slot::Value(_) => f.env().bind_variable(name.clone(), Slot::Value(loaded)),
            Slot::Cell(_) => {
                let address = f.one(
                    OpKind::Convert {
                        op: Convert::WordToAddress,
                        value: loaded,
                    },
                    Ty::Address,
                )?;
                f.env().bind_variable(name.clone(), Slot::Cell(address));
            }
        }
    }
    Ok(())
}

/// Load each required parameter into its lexical binding.
fn bind_required(
    f: &mut FunctionLowerer,
    list: &LambdaList,
    base: usize,
) -> Result<(), LowerError> {
    for (offset, name) in list.required.iter().enumerate() {
        let name = param_name(name)?;
        let index = param_index(base + offset)?;
        let loaded = f.one(OpKind::LoadArg { index }, Ty::Word)?;
        f.env().bind_variable(name, Slot::Value(loaded));
    }
    Ok(())
}

/// Lower `&aux` parameters as sequential bindings with their init forms.
fn bind_aux(
    f: &mut FunctionLowerer,
    module: &mut Lowerer,
    list: &LambdaList,
) -> Result<(), LowerError> {
    for aux in &list.aux {
        let name = param_name(&aux.name)?;
        let value = match &aux.default {
            Some(form) => lower_expr(f, module, form)?,
            None => f.nil()?,
        };
        f.env().bind_variable(name, Slot::Value(value));
    }
    Ok(())
}

/// Lower each `&optional` parameter to an `argc` test and a default block.
///
/// `argc` is the number of actual arguments the caller supplied, excluding the
/// captures and the `argc` slot, so the optional at zero-based required offset
/// `offset` is present exactly when `argc > offset`.
fn bind_optional(
    f: &mut FunctionLowerer,
    module: &mut Lowerer,
    list: &LambdaList,
    base: usize,
) -> Result<(), LowerError> {
    for (offset, optional) in list.optional.iter().enumerate() {
        let name = param_name(&optional.name)?;
        let parameter = base + list.required.len() + offset;
        let argc = load_argc(f)?;
        let threshold = f.fixnum(i64::try_from(list.required.len() + offset + 1).map_err(
            |_| LowerError::Ir {
                detail: "optional parameter index does not fit i64".to_owned(),
            },
        )?)?;
        let present = f.one(
            OpKind::Compare {
                op: Compare::Ge,
                left: argc,
                right: threshold,
            },
            Ty::Bool,
        )?;
        let branch_block = f.current_block();
        let value_slot = f.fresh_value();
        let flag_slot = f.fresh_value();
        let supplied_block = f.create_block(Vec::new());
        let default_block = f.create_block(Vec::new());
        let merge = f.create_block(vec![(Ty::Word, value_slot), (Ty::Word, flag_slot)]);
        f.position(branch_block)?;
        f.terminate(Terminator::Branch {
            condition: present,
            then_target: supplied_block,
            then_args: Vec::new(),
            else_target: default_block,
            else_args: Vec::new(),
        })?;
        f.position(default_block)?;
        let default_value = match &optional.default {
            Some(form) => lower_expr(f, module, form)?,
            None => f.nil()?,
        };
        let absent = f.fixnum(0)?;
        let absent = f.one(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: absent,
            },
            Ty::Word,
        )?;
        f.terminate(Terminator::Jump {
            target: merge,
            args: vec![default_value, absent],
        })?;
        f.position(supplied_block)?;
        let supplied_value = f.one(
            OpKind::LoadArg {
                index: param_index(parameter)?,
            },
            Ty::Word,
        )?;
        let present_flag = f.fixnum(1)?;
        let present_flag = f.one(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: present_flag,
            },
            Ty::Word,
        )?;
        f.terminate(Terminator::Jump {
            target: merge,
            args: vec![supplied_value, present_flag],
        })?;
        f.position(merge)?;
        f.env().bind_variable(name, Slot::Value(value_slot));
        if let Some(supplied_p) = &optional.supplied_p {
            let supplied_p = param_name(supplied_p)?;
            f.env().bind_variable(supplied_p, Slot::Value(flag_slot));
        }
    }
    Ok(())
}

/// Read the `argc` parameter as an `I64`.
fn load_argc(f: &mut FunctionLowerer) -> Result<ncl_ir::ValueId, LowerError> {
    let word = f.one(OpKind::LoadArg { index: 0 }, Ty::Word)?;
    f.one(
        OpKind::Convert {
            op: Convert::WordToI64,
            value: word,
        },
        Ty::I64,
    )
}

/// Convert a parameter index into the `u8` `LoadArg` accepts.
fn param_index(index: usize) -> Result<u8, LowerError> {
    u8::try_from(index).map_err(|_| LowerError::Ir {
        detail: "parameter index exceeds 255".to_owned(),
    })
}
