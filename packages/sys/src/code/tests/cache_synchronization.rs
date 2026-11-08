use super::*;

#[test]
fn cache_synchronization_requires_sealing_and_runs_once() -> Result<(), CodeError> {
    let mut code = alloc_code(1)?;
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(synchronize_code(&mut code), Err(CodeError::NotSealed));
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(seal_code(&mut code), Ok(()));
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(synchronize_code(&mut code), Ok(()));
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        synchronize_code(&mut code),
        Err(CodeError::AlreadySynchronized)
    );
    Ok(())
}
