use super::register_print_control_variables;
use ncl_object::{ObjectError, Runtime, ThreadContext, Word, with_rooted_slice};

fn register_with_released_cells(released_cells: usize) -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

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
        register_with_released_cells(0),
        Err(ObjectError::Storage(
            ncl_sys::StorageCondition::CapacityExceeded
        ))
    );
}

#[test]
fn upcase_interning_propagates_capacity_errors() {
    let mut saw_capacity_error = false;
    let mut succeeded = false;
    for released_cells in [0, 32] {
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
    let runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let mut ctx = ThreadContext::new();
    assert_eq!(
        register_print_control_variables(&runtime, &mut ctx),
        Err(ObjectError::TypeError)
    );
}
