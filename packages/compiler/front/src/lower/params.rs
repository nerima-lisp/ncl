//! Lambda parameter and capture binding for the IR v2 path.

use ncl_ir::{Compare, Constant, Convert, OpKind, Param, Terminator, Ty, ValueId};

use crate::lambda_list::{LambdaList, ParamName};
use crate::symbols::SymbolRef;

use super::super::capture;
use super::super::env::Slot;
use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::Context;

impl Context<'_> {
    pub(super) fn bind_optional(
        &mut self,
        f: &mut FunctionLowerer,
        list: &LambdaList,
        base: usize,
    ) -> Result<(), LowerError> {
        for (offset, optional) in list.optional.iter().enumerate() {
            let name = param_name(&optional.name)?;
            let argc_word = f.one(OpKind::LoadArg { index: 0 }, Ty::Word)?;
            let argc = f.one(
                OpKind::Convert {
                    op: Convert::WordToI64,
                    value: argc_word,
                },
                Ty::I64,
            )?;
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
            let branch = f.current_block();
            let value_slot = f.fresh_value();
            let flag_slot = f.fresh_value();
            let supplied = self.block(f, Vec::new());
            let default = self.block(f, Vec::new());
            let merge = self.block(f, vec![(Ty::Word, value_slot), (Ty::Word, flag_slot)]);
            f.position(branch)?;
            f.terminate(Terminator::Branch {
                condition: present,
                then_target: supplied,
                then_args: Vec::new(),
                else_target: default,
                else_args: Vec::new(),
            })?;
            f.position(default)?;
            let default_value = match optional.default.as_ref() {
                Some(form) => self.lower_expr(f, form)?,
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
            f.position(supplied)?;
            let parameter = param_index(base + list.required.len() + offset)?;
            let supplied_value = f.one(OpKind::LoadArg { index: parameter }, Ty::Word)?;
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
                f.env()
                    .bind_variable(param_name(supplied_p)?, Slot::Value(flag_slot));
            }
        }
        Ok(())
    }

    pub(super) fn bind_rest_and_keys(
        &mut self,
        f: &mut FunctionLowerer,
        list: &LambdaList,
    ) -> Result<(), LowerError> {
        let rest = if list.rest.is_some() || !list.keys.is_empty() {
            Some(Self::make_rest_list(f, list)?)
        } else {
            None
        };
        if !list.keys.is_empty() {
            let rest = rest.ok_or_else(|| LowerError::Ir {
                detail: "keyword parameters have no argument list".to_owned(),
            })?;
            let allow_other_keys = if list.allow_other_keys {
                f.word_constant(Constant::T)?
            } else {
                f.nil()?
            };
            f.safepoint()?;
            f.one(
                OpKind::Builtin {
                    name: "check-keywords".to_owned(),
                    args: vec![rest, allow_other_keys],
                },
                Ty::Word,
            )?;
            for key in &list.keys {
                let name = param_name(&key.name)?;
                let keyword = f.symbol(&key.keyword)?;
                f.safepoint()?;
                let value = f.one(
                    OpKind::Builtin {
                        name: "keyword-value".to_owned(),
                        args: vec![rest, keyword],
                    },
                    Ty::Word,
                )?;
                f.safepoint()?;
                let supplied = f.one(
                    OpKind::Builtin {
                        name: "keyword-supplied-p".to_owned(),
                        args: vec![rest, keyword],
                    },
                    Ty::Word,
                )?;
                let value = self.bind_keyword_default(f, key, value, supplied)?;
                f.env().bind_variable(name, Slot::Value(value));
                if let Some(supplied_p) = &key.supplied_p {
                    f.env()
                        .bind_variable(param_name(supplied_p)?, Slot::Value(supplied));
                }
            }
        }
        if let Some(rest_name) = &list.rest {
            f.env().bind_variable(
                param_name(rest_name)?,
                Slot::Value(rest.ok_or_else(|| LowerError::Ir {
                    detail: "rest parameter has no argument list".to_owned(),
                })?),
            );
        }
        Ok(())
    }

    pub(super) fn bind_aux(
        &mut self,
        f: &mut FunctionLowerer,
        list: &LambdaList,
    ) -> Result<(), LowerError> {
        for aux in &list.aux {
            let value = match aux.default.as_ref() {
                Some(form) => self.lower_expr(f, form)?,
                None => f.nil()?,
            };
            f.env()
                .bind_variable(param_name(&aux.name)?, Slot::Value(value));
        }
        Ok(())
    }

    fn make_rest_list(
        f: &mut FunctionLowerer,
        list: &LambdaList,
    ) -> Result<ValueId, LowerError> {
        let start = list.required.len() + list.optional.len();
        let start = i64::try_from(start).map_err(|_| LowerError::Ir {
            detail: "rest parameter index does not fit i64".to_owned(),
        })?;
        let start = f.fixnum(start)?;
        let start = f.one(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: start,
            },
            Ty::Word,
        )?;
        let argc = f.one(OpKind::LoadArg { index: 0 }, Ty::Word)?;
        f.safepoint()?;
        f.one(
            OpKind::Builtin {
                name: "make-rest-list".to_owned(),
                args: vec![argc, start],
            },
            Ty::Word,
        )
    }

    fn bind_keyword_default(
        &mut self,
        f: &mut FunctionLowerer,
        key: &crate::lambda_list::KeyParam,
        value: ValueId,
        supplied: ValueId,
    ) -> Result<ValueId, LowerError> {
        let nil = f.nil()?;
        let condition = f.one(
            OpKind::Compare {
                op: Compare::Ne,
                left: supplied,
                right: nil,
            },
            Ty::Bool,
        )?;
        let start = f.current_block();
        let present = self.block(f, Vec::new());
        let default = self.block(f, Vec::new());
        let result = f.fresh_value();
        let merge = self.block(f, vec![(Ty::Word, result)]);
        f.position(start)?;
        f.terminate(Terminator::Branch {
            condition,
            then_target: present,
            then_args: Vec::new(),
            else_target: default,
            else_args: Vec::new(),
        })?;
        f.position(present)?;
        f.terminate(Terminator::Jump {
            target: merge,
            args: vec![value],
        })?;
        f.position(default)?;
        let default_value = match key.default.as_ref() {
            Some(form) => self.lower_expr(f, form)?,
            None => f.nil()?,
        };
        f.terminate(Terminator::Jump {
            target: merge,
            args: vec![default_value],
        })?;
        f.position(merge)?;
        Ok(result)
    }
}

fn param_name(name: &ParamName) -> Result<SymbolRef, LowerError> {
    match name {
        ParamName::Symbol(symbol) => Ok(symbol.clone()),
        ParamName::Pattern(_) => Err(LowerError::UnsupportedLambdaList {
            feature: "destructuring parameter",
        }),
    }
}

pub(super) fn lambda_params(
    list: &LambdaList,
    captures: &[super::super::lambda::Capture],
) -> Result<Vec<Param>, LowerError> {
    let mut params = vec![Param {
        name: "argc".to_owned(),
        ty: Ty::Word,
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

pub(super) fn bind_captures(
    f: &mut FunctionLowerer,
    captures: &[super::super::lambda::Capture],
) -> Result<(), LowerError> {
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

pub(super) fn bind_required(
    f: &mut FunctionLowerer,
    list: &LambdaList,
    base: usize,
) -> Result<(), LowerError> {
    for (offset, name) in list.required.iter().enumerate() {
        let loaded = f.one(
            OpKind::LoadArg {
                index: param_index(base + offset)?,
            },
            Ty::Word,
        )?;
        f.env()
            .bind_variable(param_name(name)?, Slot::Value(loaded));
    }
    Ok(())
}

pub(super) fn bind_let(
    f: &mut FunctionLowerer,
    name: &SymbolRef,
    value: ValueId,
    analysis: &capture::Analysis,
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

fn param_index(index: usize) -> Result<u8, LowerError> {
    u8::try_from(index).map_err(|_| LowerError::Ir {
        detail: "parameter index exceeds 255".to_owned(),
    })
}
