use super::super::{LIST, Parameter, butlast_builtin, last_builtin, register_adapted};
use ncl_object::{
    Builtin, BuiltinConvention, BuiltinName, LambdaList, ObjectError, ParameterType, Runtime,
    ThreadContext,
};

const COUNT: Parameter = Parameter {
    name: BuiltinName::new("count"),
    ty: ParameterType::Integer,
};

pub(super) fn register(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    register_adapted(
        runtime,
        ctx,
        "LAST",
        Builtin {
            lambda_list: LambdaList::with_optional(&[LIST], &[COUNT]),
            convention: BuiltinConvention::Adapted,
        },
        last_builtin,
    )?;
    register_adapted(
        runtime,
        ctx,
        "BUTLAST",
        Builtin {
            lambda_list: LambdaList::with_optional(&[LIST], &[COUNT]),
            convention: BuiltinConvention::Adapted,
        },
        butlast_builtin,
    )
}
