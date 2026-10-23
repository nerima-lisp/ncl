use ncl_ir::{Constant, ConstantIndex};

use crate::CodegenError;

pub(super) fn constant_table_entry(
    constants: &[Constant],
    index: ConstantIndex,
) -> Result<&Constant, CodegenError> {
    let raw_index = index.0;
    let index = usize::try_from(raw_index).map_err(|_| CodegenError::InvalidConstantIndex {
        index: raw_index,
        length: constants.len(),
    })?;
    constants
        .get(index)
        .ok_or(CodegenError::InvalidConstantIndex {
            index: raw_index,
            length: constants.len(),
        })
}
