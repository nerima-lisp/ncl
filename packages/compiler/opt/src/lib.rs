//! Conservative, dependency-free optimization passes for [`ncl_ir`].

use ncl_ir::{Constant, ConstantIndex, Function, Op, OpKind, Terminator, ValueId};
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

/// The conservative first optimization: inline direct calls to small leaf functions.
#[derive(Clone, Debug)]
pub struct InlineDirectCalls {
    /// Maximum number of callee operations that may be copied.
    pub max_ops: usize,
}

impl Default for InlineDirectCalls {
    fn default() -> Self {
        Self { max_ops: 32 }
    }
}

impl InlineDirectCalls {
    fn prohibited(function: &Function) -> bool {
        !function.handler_regions.is_empty()
            || function.blocks.iter().flat_map(|b| &b.ops).any(|op| {
                matches!(
                    op.kind,
                    OpKind::MakeClosure { .. }
                        | OpKind::MakeValueCell { .. }
                        | OpKind::CallClosure { .. }
                        | OpKind::SetMultipleValues { .. }
                )
            })
    }

    fn callee_for<'a>(caller: &Function, call: &Op, module: &'a Module) -> Option<&'a Function> {
        let OpKind::Call { function, .. } = call.kind else {
            return None;
        };
        let entry = caller.blocks.iter().flat_map(|b| &b.ops).find_map(|op| {
            if op.results.len() == 1 && op.results[0].0 == function {
                match op.kind {
                    OpKind::Const { result } => usize::try_from(result.0)
                        .ok()
                        .and_then(|index| caller.constants.get(index)),
                    _ => None,
                }
            } else {
                None
            }
        })?;
        let Constant::FunctionEntry(id) = entry else {
            return None;
        };
        module
            .functions
            .iter()
            .find(|candidate| candidate.id == *id)
    }
}

impl FunctionPass for InlineDirectCalls {
    fn name(&self) -> &'static str {
        "inline-direct-calls"
    }

    fn run(&mut self, function: &mut Function, module: &Module) -> PassResult {
        if Self::prohibited(function) {
            return Ok(false);
        }
        let mut changed = false;
        let mut replacements = HashMap::new();
        let function_view = function.clone();
        let mut next_value = next_value(function);
        let caller_constants = &mut function.constants;
        for block in &mut function.blocks {
            let old_ops = std::mem::take(&mut block.ops);
            let mut new_ops = Vec::with_capacity(old_ops.len());
            for call in old_ops {
                let Some(callee) = Self::callee_for(&function_view, &call, module) else {
                    new_ops.push(call);
                    continue;
                };
                if callee.id == function.id
                    || Self::prohibited(callee)
                    || callee.blocks.len() != 1
                    || callee.blocks[0].ops.len() > self.max_ops
                    || callee.blocks[0].ops.iter().any(|op| {
                        matches!(op.kind, OpKind::Call { .. } | OpKind::CallIndirect { .. })
                    })
                {
                    new_ops.push(call);
                    continue;
                }
                let Terminator::Return { values } = &callee.blocks[0].terminator else {
                    new_ops.push(call);
                    continue;
                };
                let OpKind::Call { args, .. } = &call.kind else {
                    new_ops.push(call);
                    continue;
                };
                if args.len() != callee.params.len() || values.len() != call.results.len() {
                    new_ops.push(call);
                    continue;
                }
                let Some(mut values_map) = callee
                    .params
                    .iter()
                    .enumerate()
                    .map(|(index, _)| {
                        u32::try_from(index)
                            .ok()
                            .map(|id| (ValueId(id), args[index]))
                    })
                    .collect::<Option<HashMap<_, _>>>()
                else {
                    new_ops.push(call);
                    continue;
                };
                let mut constants = HashMap::<ConstantIndex, ConstantIndex>::new();
                for op in &callee.blocks[0].ops {
                    let kind = remap_kind(
                        &op.kind,
                        &values_map,
                        &mut constants,
                        caller_constants,
                        callee,
                    );
                    let mut results = Vec::with_capacity(op.results.len());
                    for (old, ty) in &op.results {
                        let fresh = ValueId(next_value);
                        next_value = next_value.saturating_add(1);
                        values_map.insert(*old, fresh);
                        results.push((fresh, *ty));
                    }
                    new_ops.push(Op {
                        results,
                        kind,
                        loc: op.loc,
                    });
                }
                let Some(mapped_returns) = values
                    .iter()
                    .map(|value| values_map.get(value).copied())
                    .collect::<Option<Vec<_>>>()
                else {
                    new_ops.push(call);
                    continue;
                };
                for ((result, _), mapped) in call.results.iter().zip(mapped_returns) {
                    replacements.insert(*result, mapped);
                }
                changed = true;
            }
            block.ops = new_ops;
        }
        if changed {
            for block in &mut function.blocks {
                for op in &mut block.ops {
                    remap_op_values(op, &replacements);
                }
                remap_term_values(&mut block.terminator, &replacements);
            }
        }
        Ok(changed)
    }
}

pub(crate) mod remap;
use remap::{next_value, remap_kind, remap_op_values, remap_term_values};
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
