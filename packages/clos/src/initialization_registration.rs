/// Build the adapted implementation for an initialization descriptor.
#[must_use]
pub fn implementation(descriptor: BuiltinDescriptor) -> BuiltinImplementation {
    let adapter = match descriptor.initarg_layout {
        InitArgLayout::OneLeadingArgument => initarg_adapter,
        InitArgLayout::InstanceAndSlotNames => shared_initarg_adapter,
    };
    BuiltinImplementation::adapted(descriptor.builtin, descriptor.callback, adapter)
}

/// Register the instance initialization protocol without modifying CLOS class registration.
///
/// # Errors
/// Returns an object error when builtin registration fails.
pub fn register_initialization_builtins(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for descriptor in builtin_descriptors() {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(descriptor.package, descriptor.name),
            implementation(*descriptor),
        )?;
    }
    Ok(())
}

/// Alias intended for the parent CLOS registration coordinator.
///
/// # Errors
/// Returns an object error when builtin registration fails.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    register_initialization_builtins(runtime)
}
