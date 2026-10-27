use super::{Context, FunctionLowerer, LowerError, OpKind, SymbolRef, ValueId};

impl Context<'_> {
    /// Lower calls to symbol cells into direct loads or stores where the runtime
    /// value is guaranteed to be a symbol. FDEFINITION-SET intentionally falls
    /// through to its checked builtin because its function-name argument is dynamic.
    ///
    /// Returns `Ok(None)` when `name`/`values` do not match a symbol-cell
    /// operation, so the caller falls through to ordinary call lowering.
    pub(super) fn lower_symbol_cell_call(
        &mut self,
        f: &mut FunctionLowerer,
        name: &SymbolRef,
        values: &[ValueId],
    ) -> Result<Option<ValueId>, LowerError> {
        let field =
            if matches!(name.name.as_str(), "symbol-value" | "SYMBOL-VALUE") && values.len() == 1 {
                Some((ncl_object::symbol_offset::VALUE, false))
            } else if name.name == "set-symbol-value" && values.len() == 2 {
                Some((ncl_object::symbol_offset::VALUE, true))
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
        self.load_bound_symbol_value(f, object).map(Some)
    }
}
