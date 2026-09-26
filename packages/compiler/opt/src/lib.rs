//! Conservative, dependency-free optimization passes for [`ncl_ir`].

use ncl_ir::{Constant, ConstantIndex, Function, Op, OpKind, Terminator, ValueId};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::time::{Duration, Instant};

/// A collection of functions optimized together.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Module {
    /// Functions indexed by their stable [`FunctionId`], not by vector position.
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

impl PassManager {
    /// Creates a manager with the default eight-iteration limit and verification enabled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a manager with explicit options.
    #[must_use]
    pub const fn with_options(options: PassManagerOptions) -> Self {
        Self {
            options,
            function_passes: Vec::new(),
            module_passes: Vec::new(),
        }
    }

    /// Adds a function pass in pipeline order.
    pub fn add_function_pass(&mut self, pass: impl FunctionPass + 'static) {
        self.function_passes.push(Box::new(pass));
    }

    /// Adds a module pass in pipeline order.
    pub fn add_module_pass(&mut self, pass: impl ModulePass + 'static) {
        self.module_passes.push(Box::new(pass));
    }

    /// Adds the default Phase 3 pipeline in dependency order.
    pub fn add_default_optimization_pipeline(&mut self) {
        self.add_function_pass(InlineDirectCalls::default());
        self.add_function_pass(GlobalValueNumbering);
        self.add_function_pass(Sccp);
    }

    /// Runs all passes until unchanged or until the configured limit.
    ///
    /// # Errors
    ///
    /// Returns a pass error or a post-pass verification failure.
    pub fn run(&mut self, module: &mut Module) -> Result<PassReport, PassError> {
        let mut report = PassReport::default();
        for iteration in 0..self.options.max_iterations {
            let mut changed = false;
            for pass in &mut self.function_passes {
                for index in 0..module.functions.len() {
                    let snapshot = module.clone();
                    let started = Instant::now();
                    let pass_name = pass.name().to_owned();
                    let result = pass.run(&mut module.functions[index], &snapshot);
                    let did_change =
                        result.map_err(|error| PassError::new(&pass_name, error.message))?;
                    report.stats.push(PassStat {
                        pass: pass_name.clone(),
                        changed: did_change,
                        elapsed: started.elapsed(),
                    });
                    changed |= did_change;
                    verify_after(self.options.verify_after_each_pass, &pass_name, module)?;
                }
            }
            for pass in &mut self.module_passes {
                let started = Instant::now();
                let pass_name = pass.name().to_owned();
                let did_change = pass
                    .run(module)
                    .map_err(|error| PassError::new(&pass_name, error.message))?;
                report.stats.push(PassStat {
                    pass: pass_name.clone(),
                    changed: did_change,
                    elapsed: started.elapsed(),
                });
                changed |= did_change;
                verify_after(self.options.verify_after_each_pass, &pass_name, module)?;
            }
            report.iterations = iteration + 1;
            if !changed {
                return Ok(report);
            }
        }
        report.hit_iteration_limit = self.options.max_iterations != 0;
        Ok(report)
    }
}

fn verify_after(enabled: bool, pass: &str, module: &Module) -> Result<(), PassError> {
    if enabled {
        module
            .verify()
            .map_err(|message| PassError::new(pass, message))?;
    }
    Ok(())
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
pub use gvn::GlobalValueNumbering;
mod sccp;
pub use sccp::Sccp;
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
