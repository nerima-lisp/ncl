//! Signaling `UNBOUND-VARIABLE` for a global symbol's value cell.

use ncl_ir::{Compare, Constant, OpKind, Terminator, Ty, ValueId};

use crate::symbols::SymbolRef;

use super::super::error::LowerError;
use super::super::function::FunctionLowerer;
use super::Context;

impl Context<'_> {
    /// Load a global symbol's value cell, signaling `UNBOUND-VARIABLE`
    /// instead of yielding the raw `Word::UNBOUND` sentinel when it has none.
    ///
    /// The fast path is one load and one compare-and-branch; the rare
    /// unbound case falls back to an ordinary call to
    /// `NCL-EXT:BOUND-SYMBOL-VALUE`, which performs the same check the
    /// `SYMBOL-VALUE` builtin does and carries the condition machinery
    /// (`ncl_object::bound_symbol_value`) so both paths signal identically.
    pub(super) fn load_bound_symbol_value(
        &mut self,
        f: &mut FunctionLowerer,
        symbol: ValueId,
    ) -> Result<ValueId, LowerError> {
        let value_field =
            u32::try_from(ncl_object::symbol_offset::VALUE).map_err(|_| LowerError::Ir {
                detail: "symbol value offset does not fit u32".to_owned(),
            })?;
        let loaded = f.one(
            OpKind::LoadField {
                object: symbol,
                field: value_field,
            },
            Ty::Word,
        )?;
        let unbound = f.word_constant(Constant::Unbound)?;
        let is_unbound = f.one(
            OpKind::Compare {
                op: Compare::Eq,
                left: loaded,
                right: unbound,
            },
            Ty::Bool,
        )?;
        let start = f.current_block();
        let slow = self.block(f, Vec::new());
        let fast = self.block(f, Vec::new());
        let result = f.fresh_value();
        let merge = self.block(f, vec![(Ty::Word, result)]);
        f.position(start)?;
        f.terminate(Terminator::Branch {
            condition: is_unbound,
            then_target: slow,
            then_args: Vec::new(),
            else_target: fast,
            else_args: Vec::new(),
        })?;
        f.position(fast)?;
        f.terminate(Terminator::Jump {
            target: merge,
            args: vec![loaded],
        })?;
        f.position(slow)?;
        let helper = f.symbol(&SymbolRef::interned("NCL-EXT", "BOUND-SYMBOL-VALUE"))?;
        let function_field =
            u32::try_from(ncl_object::symbol_offset::FUNCTION).map_err(|_| LowerError::Ir {
                detail: "function symbol offset does not fit u32".to_owned(),
            })?;
        let function = f.one(
            OpKind::LoadField {
                object: helper,
                field: function_field,
            },
            Ty::Word,
        )?;
        let call_argc = Self::argc(f, 1)?;
        f.safepoint()?;
        let signaled = f.one(
            OpKind::CallClosure {
                closure: function,
                args: vec![call_argc, symbol],
                named_symbol: Some(helper),
            },
            Ty::Word,
        )?;
        f.terminate(Terminator::Jump {
            target: merge,
            args: vec![signaled],
        })?;
        f.position(merge)?;
        Ok(result)
    }
}
