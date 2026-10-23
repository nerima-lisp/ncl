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
