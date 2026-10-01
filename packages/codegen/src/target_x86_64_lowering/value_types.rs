use ncl_ir::{Function, Ty, ValueId};

pub(crate) fn value_is_raw_entry(function: &Function, value: ValueId) -> bool {
    value_type(function, value) == Some(Ty::Address)
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
