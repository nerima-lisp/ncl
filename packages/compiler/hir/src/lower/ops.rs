use ncl_ir::{Constant, Convert, OpKind, Ty, ValueId};

use crate::ids::{NodeId, SymbolId};

use super::{FunctionLowerer, LowerError};

impl FunctionLowerer<'_> {
    pub(super) fn symbol_constant(&self, id: SymbolId) -> Result<Constant, LowerError> {
        let symbol = self.symbols.get(id).ok_or(LowerError::InvalidSymbol(id))?;
        match (symbol.package(), symbol.uninterned_identity()) {
            (Some(package), None) => Ok(Constant::Symbol {
                package: package.to_owned(),
                name: symbol.name().to_owned(),
            }),
            (None, Some(_)) => Ok(Constant::UninternedSymbol {
                // The source expansion identity repeats across forms; SymbolId
                // is unique throughout the compilation unit and cannot alias.
                identity: id.index(),
                name: symbol.name().to_owned(),
            }),
            (Some(_), Some(_)) | (None, None) => Err(LowerError::InvalidSymbol(id)),
        }
    }

    pub(super) fn argument_count(&mut self, count: usize) -> Result<ValueId, LowerError> {
        let count =
            i64::try_from(count).map_err(|_| LowerError::Ir("argument count overflow".into()))?;
        let raw = self.push_constant(Constant::Fixnum(count), Ty::I64)?;
        self.push_one(
            OpKind::Convert {
                op: ncl_ir::Convert::I64ToWord,
                value: raw,
            },
            Ty::Word,
        )
    }

    pub(super) fn load_field(
        &mut self,
        object: ValueId,
        field: u32,
    ) -> Result<ValueId, LowerError> {
        self.push_one(OpKind::LoadField { object, field }, Ty::Word)
    }

    pub(super) fn word_constant(&mut self, constant: Constant) -> Result<ValueId, LowerError> {
        self.push_constant(constant, Ty::Word)
    }

    pub(super) fn push_constant(
        &mut self,
        constant: Constant,
        ty: Ty,
    ) -> Result<ValueId, LowerError> {
        let index = self.add_constant(constant)?;
        self.push_one(OpKind::Const { result: index }, ty)
    }

    pub(super) fn coerce_to_word(
        &mut self,
        value: ValueId,
        conversion: Option<Convert>,
    ) -> Result<ValueId, LowerError> {
        conversion.map_or(Ok(value), |op| {
            self.push_one(OpKind::Convert { op, value }, Ty::Word)
        })
    }

    pub(super) fn add_constant(
        &mut self,
        constant: Constant,
    ) -> Result<ncl_ir::ConstantIndex, LowerError> {
        let _ = u32::try_from(self.constant_count)
            .map_err(|_| LowerError::Ir("constant table index overflow".into()))?;
        self.constant_count += 1;
        Ok(self.builder.add_constant(constant))
    }

    pub(super) fn push_one(&mut self, kind: OpKind, ty: Ty) -> Result<ValueId, LowerError> {
        let mut values = self.builder.push_op(kind, &[ty]).map_err(LowerError::Ir)?;
        values
            .pop()
            .ok_or_else(|| LowerError::Ir("operation produced no result".into()))
    }

    pub(super) fn push_none(&mut self, kind: OpKind) -> Result<(), LowerError> {
        self.builder
            .push_op(kind, &[])
            .map(|_| ())
            .map_err(LowerError::Ir)
    }

    pub(super) const fn unsupported<T>(
        node: NodeId,
        operation: &'static str,
    ) -> Result<T, LowerError> {
        Err(LowerError::Unrepresentable { node, operation })
    }
}

pub(super) const SYMBOL_VALUE_FIELD: u32 = 0;
// This is ncl_object::layout::symbol_offset::FUNCTION in the runtime ABI.
pub(super) const SYMBOL_FUNCTION_FIELD: u32 = 1;
// This is ncl_object::layout::value_cell_offset::VALUE in the runtime ABI.
pub(super) const VALUE_CELL_VALUE_FIELD: u32 = 0;
