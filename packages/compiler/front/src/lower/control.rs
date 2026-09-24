//! Lowering `block`, `return-from`, `tagbody`, and `go` to `Jump`.
//!
//! A `return-from` or `go` whose target is lexically inside the function being
//! lowered becomes a `Jump` between basic blocks. One that crosses a function
//! boundary is reported as [`LowerError::EscapingControl`]; the handler-region
//! form (`HandlerRegion` plus `Throw`) belongs to the runtime lane.

use ncl_ir::{Terminator, Ty, ValueId};

use crate::ast::{Expr, TagbodyItem};
use crate::symbols::SymbolRef;

use super::env::{BlockEntry, TagEntry};
use super::error::LowerError;
use super::expr::{lower_body, lower_expr};
use super::function::{FunctionLowerer, Lowerer};

/// Lower `(block name body*)` to an exit block its `return-from` jumps to.
pub(super) fn lower_block(
    f: &mut FunctionLowerer,
    module: &mut Lowerer,
    name: &SymbolRef,
    body: &[Expr],
) -> Result<ValueId, LowerError> {
    let result = f.fresh_value();
    let start = f.current_block();
    let exit = f.create_block(vec![(Ty::Word, result)]);
    f.position(start)?;
    f.env().push();
    f.env().bind_block(BlockEntry {
        name: name.clone(),
        target: exit,
    });
    let value = lower_body(f, module, body)?;
    f.env().pop();
    if !f.is_terminated() {
        f.terminate(Terminator::Jump {
            target: exit,
            args: vec![value],
        })?;
    }
    f.position(exit)?;
    Ok(result)
}

/// Lower `(return-from name value?)` to a `Jump` to the block's exit.
pub(super) fn lower_return_from(
    f: &mut FunctionLowerer,
    module: &mut Lowerer,
    name: &SymbolRef,
    value: Option<&Expr>,
) -> Result<ValueId, LowerError> {
    let entry = f
        .env()
        .lookup_block(name)
        .ok_or_else(|| LowerError::EscapingControl { name: name.clone() })?;
    let value = match value {
        Some(form) => lower_expr(f, module, form)?,
        None => f.nil()?,
    };
    if !f.is_terminated() {
        f.terminate(Terminator::Jump {
            target: entry.target,
            args: vec![value],
        })?;
    }
    Ok(value)
}

/// Lower `(tagbody item*)` to one block per tag and `Jump`s between them.
pub(super) fn lower_tagbody(
    f: &mut FunctionLowerer,
    module: &mut Lowerer,
    items: &[TagbodyItem],
) -> Result<ValueId, LowerError> {
    let start = f.current_block();
    let exit = f.create_block(Vec::new());
    f.position(start)?;
    f.env().push();
    let mut targets = Vec::new();
    for item in items {
        if let TagbodyItem::Tag(tag) = item {
            let block = f.create_block(Vec::new());
            targets.push((tag.clone(), block));
        }
    }
    f.position(start)?;
    for (name, target) in &targets {
        f.env().bind_tag(TagEntry {
            name: name.clone(),
            target: *target,
        });
    }
    let mut next = 0;
    for item in items {
        match item {
            TagbodyItem::Tag(_) => {
                let target = targets[next].1;
                next += 1;
                if !f.is_terminated() {
                    f.terminate(Terminator::Jump {
                        target,
                        args: Vec::new(),
                    })?;
                }
                f.position(target)?;
            }
            TagbodyItem::Form(form) => {
                if !f.is_terminated() {
                    lower_expr(f, module, form)?;
                }
            }
        }
    }
    if !f.is_terminated() {
        f.terminate(Terminator::Jump {
            target: exit,
            args: Vec::new(),
        })?;
    }
    f.position(exit)?;
    f.env().pop();
    f.nil()
}

/// Lower `(go tag)` to a `Jump` to the tag's block.
pub(super) fn lower_go(f: &mut FunctionLowerer, tag: &SymbolRef) -> Result<ValueId, LowerError> {
    let entry = f
        .env()
        .lookup_tag(tag)
        .ok_or_else(|| LowerError::EscapingControl { name: tag.clone() })?;
    let value = f.nil()?;
    if !f.is_terminated() {
        f.terminate(Terminator::Jump {
            target: entry.target,
            args: Vec::new(),
        })?;
    }
    Ok(value)
}
