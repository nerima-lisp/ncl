use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinName, BuiltinPackage,
    LambdaList, MultipleValues, ObjectError, Runtime, ThreadContext, Word,
};

pub const BUILTIN_NAME: &str = "IMAGE-ROUND-TRIP";
pub const BUILTIN_ID: BuiltinIdentifier =
    BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new(BUILTIN_NAME));
pub const BUILTIN: Builtin = Builtin {
    lambda_list: LambdaList::fixed(&[]),
    convention: BuiltinConvention::Direct(Arity::exact(0)),
};

pub const fn image_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.is_empty() {
        Ok(Word::fixnum(17))
    } else {
        Err(ObjectError::TypeError)
    }
}
