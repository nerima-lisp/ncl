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
        if matches!(name.name.as_str(), "symbol-value" | "SYMBOL-VALUE")
            && let [object] = values
        {
            return self.load_bound_symbol_value(f, *object).map(Some);
        }

        if name.name == "set-symbol-value"
            && let [object, value] = values
        {
            let field =
                u32::try_from(ncl_object::symbol_offset::VALUE).map_err(|_| LowerError::Ir {
                    detail: "symbol cell offset does not fit u32".to_owned(),
                })?;
            f.none(OpKind::StoreField {
                object: *object,
                field,
                value: *value,
            })?;
            return Ok(Some(*value));
        }

        Ok(None)
    }
}
