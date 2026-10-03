use super::{
    copy_pprint_dispatch, default_table, pprint_dispatch, print_arguments, print_error,
    set_pprint_dispatch,
};
use crate::PrintError;
use ncl_object::BuiltinArgs;
use ncl_object::{ObjectError, Runtime, ThreadContext, Word, car, cdr};

#[test]
fn dispatch_tables_match_default_and_identity_entries() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let table = default_table(&mut ctx, &runtime)?;
    let entry = car(&ctx, table)?;
    assert_eq!(car(&ctx, entry)?, Word::TRUE);
    assert_eq!(cdr(&ctx, entry)?, Word::NIL);
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(1), table)?,
        Word::NIL
    );
    let function = Word::fixnum(9);
    let table = set_pprint_dispatch(&mut ctx, &runtime, Word::fixnum(1), function, table)?;
    assert_eq!(pprint_dispatch(&mut ctx, Word::fixnum(1), table)?, function);
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(2), table)?,
        Word::NIL
    );
    Ok(())
}

#[test]
fn print_error_maps_only_object_errors() {
    assert_eq!(
        print_error(&PrintError::Object(ObjectError::Layout)),
        ObjectError::Layout
    );
    assert_eq!(print_error(&PrintError::NotReadable), ObjectError::Layout);
    assert_eq!(print_error(&PrintError::Circularity), ObjectError::Layout);
}

#[test]
fn copied_dispatch_tables_keep_order_and_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let table = default_table(&mut ctx, &runtime)?;
    let copied = copy_pprint_dispatch(&mut ctx, &runtime, table)?;
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(3), copied)?,
        Word::NIL
    );
    assert_eq!(
        print_arguments(&BuiltinArgs::new(&[Word::TRUE, Word::NIL]))?,
        vec![Word::TRUE, Word::NIL]
    );
    Ok(())
}
