#![allow(missing_docs)]

use ncl_object::{ObjectError, Package, Runtime, ThreadContext, Word, make_cons, make_string};

#[test]
fn package_registry_boundaries_preserve_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut context = ThreadContext::new();
    context.register(&runtime)?;
    let common_lisp = runtime.ensure_package(&mut context, "COMMON-LISP")?;
    assert_eq!(runtime.ensure_package(&mut context, "CL")?, common_lisp);
    assert_eq!(
        runtime.find_package(&context, "COMMON-LISP"),
        Some(common_lisp)
    );
    let package = runtime.ensure_package(&mut context, "REGISTRY-BOUNDARY")?;
    assert_eq!(
        runtime.ensure_package(&mut context, "REGISTRY-BOUNDARY")?,
        package
    );
    assert!(runtime.all_packages(&context)?.contains(&package));
    assert_eq!(runtime.class(&mut context, "REGISTRY-MISSING"), None);
    runtime.define_class(&mut context, "REGISTRY-BOUNDARY-CLASS", Word::TRUE)?;
    assert_eq!(
        runtime.class(&mut context, "REGISTRY-BOUNDARY-CLASS"),
        Some(Word::TRUE)
    );
    assert!(runtime.delete_package(&mut context, Package::from_word(package))?);
    assert_eq!(runtime.find_package(&context, "REGISTRY-BOUNDARY"), None);
    assert!(!runtime.delete_package(&mut context, Package::from_word(package))?);
    assert!(!runtime.all_packages(&context)?.contains(&package));
    Ok(())
}

#[test]
fn package_registry_rejects_malformed_names_and_lists_with_exact_errors() -> Result<(), ObjectError>
{
    let runtime = Runtime::new()?;
    let mut context = ThreadContext::new();
    context.register(&runtime)?;
    let package = runtime.ensure_package(&mut context, "REGISTRY-MALFORMED")?;
    let renamed_name = make_string(&mut context, &runtime, &['R'])?;
    assert_eq!(
        runtime.rename_package(
            &mut context,
            Package::from_word(package),
            Word::fixnum(1),
            Word::NIL
        ),
        Err(ObjectError::TypeError)
    );
    let nickname = make_string(&mut context, &runtime, &['N'])?;
    let nicknames = make_cons(&mut context, &runtime, nickname, Word::fixnum(1))?;
    assert_eq!(
        runtime.rename_package(
            &mut context,
            Package::from_word(package),
            renamed_name,
            nicknames
        ),
        Err(ObjectError::Layout)
    );
    assert_eq!(
        runtime.rename_package(
            &mut context,
            Package::from_word(package),
            renamed_name,
            Word::fixnum(1)
        ),
        Err(ObjectError::Layout)
    );
    let malformed_nickname = make_cons(&mut context, &runtime, Word::fixnum(1), Word::NIL)?;
    assert_eq!(
        runtime.rename_package(
            &mut context,
            Package::from_word(package),
            renamed_name,
            malformed_nickname
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}
