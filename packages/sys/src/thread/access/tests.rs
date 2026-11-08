use crate::{CodeError, Thread, ThreadAccess, alloc_code, publish_code};

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[test]
fn thread_access_invokes_with_the_explicit_function_object() -> Result<(), CodeError> {
    #[cfg(target_arch = "aarch64")]
    let bytes = [0xe0, 0x03, 0x10, 0xaa, 0xc0, 0x03, 0x5f, 0xd6];
    #[cfg(target_arch = "x86_64")]
    let bytes = [0x4c, 0x89, 0xd0, 0xc3];

    let mut code = alloc_code(bytes.len())?;
    code.write_code(0, &bytes)?;
    publish_code(&mut code)?;

    let function_object = 0x1234_5678_9abc_def0_u64;
    let mut thread = Thread::new();
    let mut access = ThreadAccess::new(&mut thread);
    let (returned_function, _) =
        access.invoke_entry_with_function(&code, 0, function_object, 0, [0; 4], 0);
    assert_eq!(returned_function, function_object);
    Ok(())
}
