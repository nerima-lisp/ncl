use super::RuntimeError;

/// Whether runtime compilation applies the default optimization pipeline.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OptimizationMode {
    /// Apply the GC-safe default optimization passes.
    #[default]
    Default,
    /// Compile verified frontend IR without optimization passes.
    Off,
}

/// Options controlling optimization and its diagnostics for one runtime.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeOptions {
    /// Optimization mode used for every compiled form.
    pub optimization: OptimizationMode,
    /// Emit one diagnostic line for each pass invocation to stderr.
    pub opt_diagnostics: bool,
}

impl RuntimeOptions {
    /// Creates options with an explicit mode and diagnostic setting.
    #[must_use]
    pub const fn new(optimization: OptimizationMode, opt_diagnostics: bool) -> Self {
        Self {
            optimization,
            opt_diagnostics,
        }
    }
}

pub fn verify_and_optimize(
    module: &mut ncl_opt::Module,
    options: RuntimeOptions,
) -> Result<(), RuntimeError> {
    module.verify().map_err(|error| {
        RuntimeError::Native(format!(
            "IR verification before optimization failed: {error}"
        ))
    })?;
    if options.optimization == OptimizationMode::Default {
        let mut passes = ncl_opt::PassManager::new();
        passes.add_default_optimization_pipeline();
        let result = if options.opt_diagnostics {
            passes.run_with_observer(module, |event| eprintln!("{event}"))
        } else {
            passes.run(module)
        };
        result.map_err(|error| RuntimeError::Native(error.to_string()))?;
    }
    Ok(())
}
