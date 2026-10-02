use ncl_ir::{Constant, Function, OpKind, Ty, ValueId};

pub fn value_is_raw_entry(function: &Function, value: ValueId) -> bool {
    value_type(function, value) == Some(Ty::Address) || is_function_entry(function, value)
}

fn is_function_entry(function: &Function, value: ValueId) -> bool {
    function.blocks.iter().any(|block| {
        block.ops.iter().any(|op| {
            let OpKind::Const { result } = &op.kind else {
                return false;
            };
            let Some(index) = usize::try_from(result.0).ok() else {
                return false;
            };
            op.results.iter().any(|(candidate, _)| *candidate == value)
                && matches!(
                    function.constants.get(index),
                    Some(Constant::FunctionEntry(_))
                )
        })
    })
}

fn value_type(function: &Function, value: ValueId) -> Option<Ty> {
    function
        .params
        .get(usize::try_from(value.0).ok()?)
        .map(|param| param.ty)
        .or_else(|| {
            function.blocks.iter().find_map(|block| {
                block
                    .params
                    .iter()
                    .find(|param| param.value == value)
                    .map(|param| param.ty)
                    .or_else(|| {
                        block.ops.iter().find_map(|op| {
                            op.results
                                .iter()
                                .find(|(result, _)| *result == value)
                                .map(|(_, ty)| *ty)
                        })
                    })
            })
        })
}

#[cfg(test)]
mod tests {
    use super::value_is_raw_entry;
    use ncl_ir::{Constant, FunctionBuilder, OpKind, Ty};

    #[test]
    fn function_entry_constants_are_raw_call_targets() -> Result<(), String> {
        let mut builder = FunctionBuilder::new(
            ncl_ir::FunctionId(1),
            "raw-entry",
            Vec::new(),
            vec![Ty::Word],
        );
        let entry = builder.add_constant(Constant::FunctionEntry(ncl_ir::FunctionId(2)));
        let value = builder
            .push_op(OpKind::Const { result: entry }, &[Ty::Word])?
            .into_iter()
            .next()
            .ok_or_else(|| "function entry value missing".to_owned())?;
        builder.terminate(ncl_ir::Terminator::Return {
            values: vec![value],
        })?;

        assert!(value_is_raw_entry(&builder.finish(), value));
        Ok(())
    }

    #[test]
    fn address_values_are_raw_and_unknown_or_word_values_are_not() -> Result<(), String> {
        let mut builder = FunctionBuilder::new(
            ncl_ir::FunctionId(218),
            "raw-address-values",
            vec![ncl_ir::Param {
                name: "address".into(),
                ty: Ty::Address,
            }],
            Vec::new(),
        );
        let block_value = ncl_ir::ValueId(7);
        builder.create_block(vec![(Ty::Address, block_value)]);
        let function = builder.finish();
        assert!(value_is_raw_entry(&function, ncl_ir::ValueId(0)));
        assert!(value_is_raw_entry(&function, block_value));
        assert!(!value_is_raw_entry(&function, ncl_ir::ValueId(99)));

        let mut word_builder = FunctionBuilder::new(
            ncl_ir::FunctionId(219),
            "word-value",
            Vec::new(),
            vec![Ty::Word],
        );
        let constant = word_builder.add_constant(Constant::Fixnum(1));
        let value = word_builder
            .push_op(OpKind::Const { result: constant }, &[Ty::Word])?
            .into_iter()
            .next()
            .ok_or_else(|| "word result missing".to_owned())?;
        word_builder.terminate(ncl_ir::Terminator::Return {
            values: vec![value],
        })?;
        assert!(!value_is_raw_entry(&word_builder.finish(), value));
        Ok(())
    }
}
