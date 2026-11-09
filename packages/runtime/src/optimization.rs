use super::{Runtime, RuntimeError};

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

pub type PassRunner = dyn FnMut(
    &mut ncl_opt::PassManager,
    &mut ncl_opt::Module,
) -> Result<ncl_opt::PassReport, ncl_opt::PassError>;

pub struct StoredPassRunner(pub Box<PassRunner>);

impl std::fmt::Debug for StoredPassRunner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("StoredPassRunner")
    }
}

pub fn default_pass_runner(
    options: RuntimeOptions,
) -> impl FnMut(
    &mut ncl_opt::PassManager,
    &mut ncl_opt::Module,
) -> Result<ncl_opt::PassReport, ncl_opt::PassError> {
    move |passes, module| {
        if options.opt_diagnostics {
            passes.run_with_observer(module, |event| eprintln!("{event}"))
        } else {
            passes.run(module)
        }
    }
}

impl Runtime {
    /// Create a runtime that delegates each default pass pipeline to `pass_runner`.
    ///
    /// The runtime verifies each module and prepares the default pipeline before it calls the
    /// runner. The runner receives a mutable pass manager and module for each compiled form.
    ///
    /// # Errors
    /// Returns an initialization error from the object or standard-library layers. A pass error
    /// is returned by the compilation or evaluation call that invoked the runner.
    pub fn with_options_and_pass_runner<F>(
        options: RuntimeOptions,
        pass_runner: F,
    ) -> Result<Self, RuntimeError>
    where
        F: FnMut(
                &mut ncl_opt::PassManager,
                &mut ncl_opt::Module,
            ) -> Result<ncl_opt::PassReport, ncl_opt::PassError>
            + 'static,
    {
        let mut runtime = Self::with_options(options)?;
        runtime.pass_runner = Some(StoredPassRunner(Box::new(pass_runner)));
        Ok(runtime)
    }
}

pub fn verify_and_optimize(
    module: &mut ncl_opt::Module,
    options: RuntimeOptions,
    pass_runner: &mut PassRunner,
) -> Result<(), RuntimeError> {
    module.verify().map_err(|error| {
        RuntimeError::Native(format!(
            "IR verification before optimization failed: {error}"
        ))
    })?;
    if options.optimization == OptimizationMode::Default {
        let mut passes = ncl_opt::PassManager::new();
        passes.add_default_optimization_pipeline();
        pass_runner(&mut passes, module)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{OptimizationMode, RuntimeOptions};

    #[test]
    fn pass_diagnostics_are_disabled_by_default() {
        let options = RuntimeOptions::default();
        assert!(!options.opt_diagnostics);
        assert!(RuntimeOptions::new(OptimizationMode::Default, true).opt_diagnostics);
    }
}

#[cfg(test)]
#[path = "tests/optimization.rs"]
mod coverage_tests;
