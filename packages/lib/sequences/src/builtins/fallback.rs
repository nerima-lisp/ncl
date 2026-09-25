pub(crate) fn unsupported(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Err(ObjectError::Unsupported)
}
pub(crate) fn passthrough(_args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    Err(ObjectError::Unsupported)
}
pub(crate) fn unsupported_implementation() -> BuiltinImplementation {
    BuiltinImplementation::adapted(
        Builtin {
            lambda_list: LambdaList::with_rest(&[], parameter("arguments", ParameterType::Any)),
            convention: BuiltinConvention::Adapted,
        },
        unsupported,
        passthrough,
    )
}
pub(crate) fn identifier(name: &'static str) -> BuiltinIdentifier {
    BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name))
}

