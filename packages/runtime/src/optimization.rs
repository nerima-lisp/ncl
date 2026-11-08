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
    /// Measure successful Rust allocator request bytes around each pass body.
    pub pass_allocation_diagnostics: bool,
}

impl RuntimeOptions {
    /// Creates options with an explicit mode and diagnostic setting.
    #[must_use]
    pub const fn new(optimization: OptimizationMode, opt_diagnostics: bool) -> Self {
        Self {
            optimization,
            opt_diagnostics,
            pass_allocation_diagnostics: false,
        }
    }

    /// Enable pass allocator counters for an instrumented profiling executable.
    #[must_use]
    pub const fn with_pass_allocation_diagnostics(mut self, enabled: bool) -> Self {
        self.pass_allocation_diagnostics = enabled;
        self.opt_diagnostics |= enabled;
        self
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
            if options.pass_allocation_diagnostics {
                let scope = ncl_sys::AllocationTrackingScope::start()
                    .map_err(|error| RuntimeError::Native(error.to_string()))?;
                passes.run_with_observer_and_allocation_snapshot(
                    module,
                    |event| eprintln!("{event}"),
                    || scope.allocated_bytes(),
                )
            } else {
                passes.run_with_observer(module, |event| eprintln!("{event}"))
            }
        } else {
            passes.run(module)
        };
        result.map_err(|error| RuntimeError::Native(error.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{OptimizationMode, RuntimeOptions, verify_and_optimize};
    use crate::RuntimeError;

    #[test]
    fn allocation_diagnostics_are_disabled_by_default() {
        let options = RuntimeOptions::default();
        assert!(!options.opt_diagnostics);
        assert!(!options.pass_allocation_diagnostics);
        assert!(!RuntimeOptions::new(OptimizationMode::Default, true).pass_allocation_diagnostics);
    }

    #[test]
    fn allocation_diagnostics_fail_without_the_profiler_allocator() {
        let options = RuntimeOptions::default().with_pass_allocation_diagnostics(true);
        let result = verify_and_optimize(&mut ncl_opt::Module::default(), options);
        assert!(matches!(
            result,
            Err(RuntimeError::Native(message)) if message == "allocation tracker is not installed"
        ));
    }

    #[test]
    fn ncl_gc_allocation_snapshots_are_callable_from_lisp() {
        let mut runtime = crate::Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
        let gc_count = runtime
            .eval("(NCL-GC:GC-COUNT)")
            .unwrap_or_else(|error| panic!("GC-COUNT: {error}"));
        assert!(gc_count.as_fixnum().is_some_and(|value| value >= 0));
        let bytes = runtime
            .eval("(NCL-GC:BYTES-CONSED-SINCE-GC)")
            .unwrap_or_else(|error| panic!("BYTES-CONSED-SINCE-GC: {error}"));
        assert!(bytes.as_fixnum().is_some_and(|value| value >= 0));
    }
}

#[cfg(test)]
#[path = "tests/optimization.rs"]
mod coverage_tests;
