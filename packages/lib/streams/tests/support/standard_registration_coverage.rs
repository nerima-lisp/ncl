use super::{register_print_control_variable, register_print_control_variables};
use ncl_object::{ObjectError, Package, Runtime, ThreadContext, Word, with_rooted_slice};

const PRINT_CONTROL_VARIABLES: [&str; 12] = [
    "*PRINT-ARRAY*",
    "*PRINT-BASE*",
    "*PRINT-CIRCLE*",
    "*PRINT-ESCAPE*",
    "*PRINT-GENSYM*",
    "*PRINT-LENGTH*",
    "*PRINT-LEVEL*",
    "*PRINT-LINES*",
    "*PRINT-MISER-WIDTH*",
    "*PRINT-PRETTY*",
    "*PRINT-RADIX*",
    "*PRINT-RIGHT-MARGIN*",
];

#[derive(Clone, Copy)]
enum Setup {
    FirstPrintVariable,
    MissingUpcase,
    MissingKeywordPackage,
}

fn register_with_released_cells(setup: Setup, released_cells: usize) -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    match setup {
        Setup::FirstPrintVariable => {}
        Setup::MissingUpcase | Setup::MissingKeywordPackage => {
            for name in PRINT_CONTROL_VARIABLES {
                let package = runtime.ensure_package(&mut ctx, "COMMON-LISP")?;
                register_print_control_variable(&mut ctx, &runtime, package, name, Word::NIL)?;
            }
            if matches!(setup, Setup::MissingUpcase) {
                let keyword = runtime.ensure_package(&mut ctx, "KEYWORD")?;
                let (upcase, _) =
                    Package::from_word(keyword).intern(&mut ctx, &runtime, "UPCASE")?;
                let name = ncl_object::symbol_name(&ctx, upcase)?;
                let keyword = runtime.ensure_package(&mut ctx, "KEYWORD")?;
                let removed = Package::from_word(keyword).unintern(&mut ctx, &runtime, name)?;
                assert!(removed);
            } else {
                let keyword = runtime.ensure_package(&mut ctx, "KEYWORD")?;
                let removed = runtime.delete_package(&mut ctx, Package::from_word(keyword))?;
                assert!(removed);
                assert!(runtime.find_package(&ctx, "KEYWORD").is_none());
            }
        }
    }

    let mut roots = vec![Word::NIL; 1_024];
    with_rooted_slice(&mut ctx, &mut roots, |ctx, roots| {
        let mut next_root = 0;
        for words in [65_536, 4_096, 256, 16, 1] {
            loop {
                match ncl_object::allocate(ctx, &runtime, 0x7f, words) {
                    Ok(value) => {
                        roots[next_root] = value;
                        next_root += 1;
                    }
                    Err(ObjectError::Storage(ncl_sys::StorageCondition::CapacityExceeded)) => break,
                    Err(error) => return Err(error),
                }
            }
        }

        for root in &mut roots[next_root - released_cells..next_root] {
            *root = Word::NIL;
        }
        ctx.collect(true)?;
        register_print_control_variables(&runtime, ctx)
    })
}

#[test]
fn print_control_registration_propagates_capacity_errors() {
    assert_eq!(
        register_with_released_cells(Setup::FirstPrintVariable, 0),
        Err(ObjectError::Storage(
            ncl_sys::StorageCondition::CapacityExceeded
        ))
    );
}

#[test]
fn upcase_interning_propagates_capacity_errors() {
    let mut saw_capacity_error = false;
    let mut succeeded = false;
    for released_cells in 0..=32 {
        match register_with_released_cells(Setup::MissingUpcase, released_cells) {
            Err(ObjectError::Storage(ncl_sys::StorageCondition::CapacityExceeded)) => {
                saw_capacity_error = true;
            }
            Ok(()) => {
                succeeded = true;
                break;
            }
            Err(error) => panic!("unexpected registration error: {error:?}"),
        }
    }
    assert!(saw_capacity_error);
    assert!(
        succeeded,
        "registration should succeed with sufficient capacity"
    );
}

#[test]
fn keyword_package_creation_propagates_capacity_errors() {
    let mut saw_capacity_error = false;
    let mut succeeded = false;
    for released_cells in 0..=32 {
        match register_with_released_cells(Setup::MissingKeywordPackage, released_cells) {
            Err(ObjectError::Storage(ncl_sys::StorageCondition::CapacityExceeded)) => {
                saw_capacity_error = true;
            }
            Ok(()) => {
                succeeded = true;
                break;
            }
            Err(error) => panic!("unexpected registration error: {error:?}"),
        }
    }
    assert!(saw_capacity_error);
    assert!(
        succeeded,
        "registration should succeed with sufficient capacity"
    );
}

#[test]
fn print_control_registration_requires_a_registered_context() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    assert_eq!(
        register_print_control_variables(&runtime, &mut ctx),
        Err(ObjectError::TypeError)
    );
}
