//! Escaping control transfer lowering for the IR v2 path.

use ncl_ir::{Convert, OpKind, Terminator, Ty, ValueId};

use crate::symbols::SymbolRef;

use super::super::env::Slot;
use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::Context;
use super::NonLocalTarget;

impl Context<'_> {
    pub(super) fn fresh_token(f: &mut FunctionLowerer) -> Result<ValueId, LowerError> {
        let symbol = f.symbol(&SymbolRef::interned("COMMON-LISP", "GENSYM"))?;
        let function = f.one(
            OpKind::LoadField {
                object: symbol,
                field: u32::try_from(ncl_object::symbol_offset::FUNCTION).map_err(|_| {
                    LowerError::Ir {
                        detail: "gensym function offset does not fit u32".to_owned(),
                    }
                })?,
            },
            Ty::Word,
        )?;
        let zero = f.fixnum(0)?;
        let argc = f.one(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: zero,
            },
            Ty::Word,
        )?;
        f.safepoint()?;
        f.one(
            OpKind::CallClosure {
                closure: function,
                args: vec![argc],
                named_symbol: None,
            },
            Ty::Word,
        )
    }

    pub(super) fn target_value(
        f: &mut FunctionLowerer,
        target: &NonLocalTarget,
        name: &SymbolRef,
    ) -> Result<ValueId, LowerError> {
        match f.env().lookup_variable(&target.capture) {
            // check-added-lines: allow(wildcard) both bindings carry a word pointer.
            Some(Slot::Value(value) | Slot::Cell(value)) => Ok(value),
            None => Err(LowerError::EscapingControl { name: name.clone() }),
        }
    }

    pub(super) fn lower_go(
        &mut self,
        f: &mut FunctionLowerer,
        tag: &SymbolRef,
    ) -> Result<ValueId, LowerError> {
        if let Some(entry) = f.env().lookup_tag(tag) {
            let value = f.nil()?;
            if !f.is_terminated() {
                self.close_active_since(f, entry.active_depth)?;
            }
            if !f.is_terminated() {
                let args = entry
                    .live
                    .iter()
                    .filter_map(|name| match f.env().lookup_variable(name) {
                        Some(Slot::Value(value)) => Some(value),
                        // check-added-lines: allow(wildcard) only value slots are jump arguments.
                        _ => None,
                    })
                    .collect();
                Self::terminate_jump(f, entry.target, args)?;
            }
            return Ok(value);
        }
        let target = self
            .target(tag)
            .ok_or_else(|| LowerError::EscapingControl { name: tag.clone() })?;
        let value = f.nil()?;
        let token = Self::target_value(f, &target, tag)?;
        f.safepoint()?;
        let thrown = f.one(
            OpKind::Builtin {
                name: "throw".to_owned(),
                args: vec![token, value],
            },
            Ty::Word,
        )?;
        if !f.is_terminated() {
            f.terminate(Terminator::Throw { condition: thrown })?;
        }
        Ok(value)
    }
}
