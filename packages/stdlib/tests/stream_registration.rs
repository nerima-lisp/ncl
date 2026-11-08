//! Package-operation failures reached through standard stream registration.

#![expect(
    clippy::unwrap_used,
    reason = "the fixtures unwrap setup and registration errors to fail immediately"
)]

use ncl_object::{FunctionObject, ObjectError, Runtime, ThreadContext, Word, with_rooted_slice};

#[derive(Clone, Copy)]
enum Setup {
    MissingUpcase,
    MissingKeywordPackage,
}

fn call_package_builtin(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("{name} builtin is not registered"));
    runtime.call_builtin(ctx, function, args)
}

fn register_with_released_cells(setup: Setup, released_cells: usize) -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_stdlib::register_all(&mut ctx, &runtime)?;

    let keyword = runtime
        .find_package(&ctx, "KEYWORD")
        .unwrap_or_else(|| panic!("KEYWORD package is not registered"));
    match setup {
        Setup::MissingUpcase => {
            let name =
                ncl_object::make_string(&mut ctx, &runtime, &['U', 'P', 'C', 'A', 'S', 'E'])?;
            assert_eq!(
                call_package_builtin(&runtime, &mut ctx, "UNINTERN", &[name, keyword])?,
                Word::TRUE
            );
        }
        Setup::MissingKeywordPackage => {
            assert_eq!(
                call_package_builtin(&runtime, &mut ctx, "DELETE-PACKAGE", &[keyword])?,
                Word::TRUE
            );
            assert!(runtime.find_package(&ctx, "KEYWORD").is_none());
        }
    }

    let roots = vec![Word::NIL; 1_024];
    with_rooted_slice(&mut ctx, &roots, |ctx, roots| {
        let mut next_root = 0;
        for words in [65_536, 4_096, 256, 16, 1] {
            loop {
                match ncl_object::allocate(ctx, &runtime, 0x7f, words) {
                    Ok(value) => {
                        roots[next_root] = value;
                        next_root += 1;
                    }
                    Err(ObjectError::Storage(ncl_sys::StorageCondition::CapacityExceeded)) => {
                        break;
                    }
                    Err(error) => return Err(error),
                }
            }
        }

        for root in &mut roots[next_root - released_cells..next_root] {
            *root = Word::NIL;
        }
        ctx.collect(true)?;
        ncl_lib_streams::register_with_context(&runtime, ctx)
    })
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
