#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert on image capture failures"
)]

use super::{Capture, Record};
use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError, Runtime, ThreadContext,
    Word,
};

const BUILTIN_ID: BuiltinIdentifier =
    BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("SAVE-ENTRY-ZERO"));

const BUILTIN: Builtin = Builtin {
    lambda_list: LambdaList::fixed(&[]),
    convention: BuiltinConvention::Direct(Arity::exact(0)),
};

const fn image_builtin(
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

#[test]
fn registered_builtin_capture_zeros_the_entry() -> Result<(), crate::ImageError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let function = runtime.register_builtin(
        &mut ctx,
        BUILTIN_ID,
        BuiltinImplementation::direct(BUILTIN, image_builtin).with_entry(0x1234_5678),
    )?;

    let mut capture = Capture::new(&runtime);
    capture.run(&mut ctx, &[function.as_word()])?;
    let records = capture.finish();

    assert!(records.iter().any(|record| matches!(
        record,
        Record::Function {
            entry: 0,
            builtin: Some(identity),
            ..
        } if identity.package == "NCL-TEST" && identity.name == "SAVE-ENTRY-ZERO"
    )));
    Ok(())
}
