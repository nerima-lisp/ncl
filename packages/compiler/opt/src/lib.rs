//! Conservative, dependency-free optimization passes for [`ncl_ir`].

use ncl_ir::{Function, Ty, ValueId};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::time::Duration;

/// A collection of functions optimized together.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Module {
    /// Functions indexed by their stable [`ncl_ir::FunctionId`], not by vector position.
    pub functions: Vec<Function>,
}

impl Module {
    /// Verifies every function in the module.
    ///
    /// # Errors
    ///
    /// Returns the first function verification failure.
    pub fn verify(&self) -> Result<(), String> {
        for function in &self.functions {
            ncl_ir::verify(function)
                .map_err(|errors| format!("function {}: {errors:?}", function.name))?;
        }
        Ok(())
    }
}

fn value_types(function: &Function) -> HashMap<ValueId, Ty> {
    let mut types = HashMap::new();
    for (index, parameter) in function.params.iter().enumerate() {
        if let Ok(index) = u32::try_from(index) {
            types.insert(ValueId(index), parameter.ty);
        }
    }
    for block in &function.blocks {
        for parameter in &block.params {
            types.insert(parameter.value, parameter.ty);
        }
        for op in &block.ops {
            types.extend(op.results.iter().map(|(value, ty)| (*value, *ty)));
        }
    }
    types
}

const fn proven_scalar_word(fact: Option<&ScalarFact>) -> bool {
    matches!(
        fact,
        Some(ScalarFact::Known {
            ty: ScalarType::Fixnum | ScalarType::Boolean,
            evidence: Evidence::Proven,
        })
    )
}

pub(crate) fn is_scalar_value(
    types: &HashMap<ValueId, Ty>,
    facts: &FunctionFacts,
    value: ValueId,
) -> bool {
    match types.get(&value) {
        Some(Ty::I64 | Ty::F64 | Ty::Bool) => true,
        Some(Ty::Word) => proven_scalar_word(facts.values.get(&value)),
        Some(Ty::Address | Ty::Unit) | None => false,
    }
}

/// The result of one pass invocation.
pub type PassResult = Result<bool, PassError>;

/// A pass over one function.
pub trait FunctionPass: Send + std::fmt::Debug {
    /// The stable human-readable pass name.
    fn name(&self) -> &'static str;
    /// Runs the pass and returns whether the function changed.
    ///
    /// # Errors
    ///
    /// Returns a pass-specific transformation error.
    fn run(&mut self, function: &mut Function, module: &Module) -> PassResult;
}

/// A pass over a complete module.
pub trait ModulePass: Send + std::fmt::Debug {
    /// The stable human-readable pass name.
    fn name(&self) -> &'static str;
    /// Runs the pass and returns whether the module changed.
    ///
    /// # Errors
    ///
    /// Returns a pass-specific transformation error.
    fn run(&mut self, module: &mut Module) -> PassResult;
}

/// A pass failure. The pass name is retained in both the value and its display text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassError {
    /// Name of the pass that failed.
    pub pass: String,
    /// Detail supplied by the pass or verifier.
    pub message: String,
}

impl PassError {
    fn new(pass: &str, message: impl Into<String>) -> Self {
        Self {
            pass: pass.into(),
            message: message.into(),
        }
    }
}

impl Display for PassError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "pass {} failed: {}", self.pass, self.message)
    }
}
impl Error for PassError {}

/// Whether the module verifier ran after one pass invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PassVerification {
    /// Verification did not run because the pass itself failed.
    NotRun,
    /// Verification was disabled for this manager.
    Skipped,
    /// The module passed verification.
    Passed,
    /// Verification rejected the module.
    Failed(String),
    /// Verification rejected the pass output and the module was restored.
    RolledBack(String),
}

/// Details for one pass invocation, including failed invocations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassEvent {
    /// One-based fixed-point iteration.
    pub iteration: usize,
    /// Name of the function, or `None` for a module pass.
    pub function: Option<String>,
    /// Pass name.
    pub pass: String,
    /// Whether the pass changed the module, or `None` if it failed.
    pub changed: Option<bool>,
    /// Verification outcome after this invocation.
    pub verification: PassVerification,
    /// Wall-clock time spent running the pass, in nanoseconds.
    pub elapsed: Duration,
    /// Pass or verification failure detail.
    pub error: Option<String>,
}

impl Display for PassEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ncl-opt iteration={} function={} pass={:?} changed={} verification=",
            self.iteration,
            self.function
                .as_deref()
                .map_or_else(|| "-".to_owned(), |name| format!("{name:?}")),
            self.pass,
            self.changed
                .map_or_else(|| "failed".to_owned(), |changed| changed.to_string()),
        )?;
        match &self.verification {
            PassVerification::NotRun => f.write_str("not-run")?,
            PassVerification::Skipped => f.write_str("skipped")?,
            PassVerification::Passed => f.write_str("passed")?,
            PassVerification::Failed(_) => f.write_str("failed")?,
            PassVerification::RolledBack(_) => f.write_str("rolled-back")?,
        }
        write!(f, " elapsed_ns={}", self.elapsed.as_nanos())?;
        if let Some(error) = &self.error {
            write!(f, " error={error:?}")?;
        }
        Ok(())
    }
}

/// One timed pass invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassStat {
    /// Pass name.
    pub pass: String,
    /// Whether this invocation changed its input.
    pub changed: bool,
    /// Wall-clock time spent in the pass, in nanoseconds.
    pub elapsed: Duration,
}

/// Results and statistics from a manager run.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PassReport {
    /// Number of fixed-point iterations performed.
    pub iterations: usize,
    /// Whether the iteration limit stopped the run before a fixed point.
    pub hit_iteration_limit: bool,
    /// Per-invocation change and timing statistics.
    pub stats: Vec<PassStat>,
}

/// Configuration for [`PassManager`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassManagerOptions {
    /// Maximum number of complete pass-pipeline iterations.
    pub max_iterations: usize,
    /// Whether to verify the whole module after every pass invocation.
    pub verify_after_each_pass: bool,
}

impl Default for PassManagerOptions {
    fn default() -> Self {
        Self {
            max_iterations: 8,
            verify_after_each_pass: true,
        }
    }
}

impl PassManagerOptions {
    /// Disables the explicit post-pass verification checks.
    #[must_use]
    pub const fn without_verification(mut self) -> Self {
        self.verify_after_each_pass = false;
        self
    }
}

/// Runs function and module passes to a fixed point.
#[derive(Debug, Default)]
pub struct PassManager {
    options: PassManagerOptions,
    function_passes: Vec<Box<dyn FunctionPass>>,
    module_passes: Vec<Box<dyn ModulePass>>,
}

mod inline;
pub(crate) mod remap;
pub use inline::InlineDirectCalls;
mod gvn;
mod manager;
pub use gvn::GlobalValueNumbering;
mod sccp;
pub use sccp::Sccp;
mod dce;
pub use dce::DeadCodeElimination;
mod type_effect;
pub use type_effect::{
    Effect, EffectSet, Evidence, FunctionFacts, ScalarFact, ScalarType, TypeEffect, ValuesArity,
};
#[cfg(test)]
#[path = "dce_tests.rs"]
mod dce_tests;
#[cfg(test)]
#[path = "gvn_tests.rs"]
mod gvn_tests;
#[cfg(test)]
#[path = "manager_tests.rs"]
mod manager_tests;
#[cfg(test)]
#[path = "remap_tests.rs"]
mod remap_tests;
#[cfg(test)]
#[path = "sccp_tests.rs"]
mod sccp_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
#[cfg(test)]
#[path = "tests_support.rs"]
mod tests_support;
