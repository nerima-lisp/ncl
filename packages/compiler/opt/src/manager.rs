use super::{
    DeadCodeElimination, FunctionPass, GlobalValueNumbering, InlineDirectCalls, Module, ModulePass,
    PassError, PassEvent, PassManager, PassManagerOptions, PassReport, PassStat, PassVerification,
    Sccp, TypeEffect,
};
mod allocation_counter;
use allocation_counter::{AllocationCounter, DisabledAllocationCounter, SnapshotAllocationCounter};
mod pass_execution;
mod provenance;
use pass_execution::{PassRunContext, run_function_pass, run_module_pass};
use std::collections::HashSet;

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

    /// Adds SCCP, type/effect analysis, scalar inlining, scalar GVN, and DCE in order.
    pub fn add_default_optimization_pipeline(&mut self) {
        self.add_function_pass(Sccp);
        self.add_function_pass(TypeEffect);
        self.add_function_pass(InlineDirectCalls::default());
        self.add_function_pass(GlobalValueNumbering);
        self.add_function_pass(DeadCodeElimination);
    }

    /// Runs all passes until unchanged or until the configured limit.
    ///
    /// # Errors
    ///
    /// Returns a pass error or a post-pass verification failure.
    pub fn run(&mut self, module: &mut Module) -> Result<PassReport, PassError> {
        self.run_with_observer(module, |_| {})
    }

    /// Runs passes to a fixed point and reports each invocation to `observer`.
    ///
    /// # Errors
    ///
    /// Returns a pass error or a post-pass verification failure.
    pub fn run_with_observer(
        &mut self,
        module: &mut Module,
        observer: impl FnMut(&PassEvent),
    ) -> Result<PassReport, PassError> {
        self.run_with_counter(module, observer, &mut DisabledAllocationCounter)
    }

    /// Runs passes and measures successful allocator requests around each pass body.
    ///
    /// The snapshot callback must read a monotonically increasing byte counter on
    /// the pass-running thread. A missing or decreasing snapshot omits that value.
    ///
    /// # Errors
    /// Returns a pass error or a post-pass verification failure.
    pub fn run_with_observer_and_allocation_snapshot(
        &mut self,
        module: &mut Module,
        observer: impl FnMut(&PassEvent),
        snapshot: impl FnMut() -> Option<u128>,
    ) -> Result<PassReport, PassError> {
        let mut counter = SnapshotAllocationCounter::new(snapshot);
        self.run_with_counter(module, observer, &mut counter)
    }

    fn run_with_counter<C: AllocationCounter>(
        &mut self,
        module: &mut Module,
        mut observer: impl FnMut(&PassEvent),
        counter: &mut C,
    ) -> Result<PassReport, PassError> {
        let snapshot = module.clone();
        let result = (|| {
            let mut report = PassReport::default();
            let verify = self.options.verify_after_each_pass;
            let mut skipped_functions = HashSet::new();
            let mut skipped_module_passes = HashSet::new();
            for iteration in 0..self.options.max_iterations {
                let mut changed = false;
                for (pass_index, pass) in self.function_passes.iter_mut().enumerate() {
                    for index in 0..module.functions.len() {
                        let Some(function_name) = module
                            .functions
                            .get(index)
                            .map(|function| function.name.clone())
                        else {
                            return Err(PassError::new(
                                "pass manager",
                                "function index out of bounds",
                            ));
                        };
                        let skip_key = (pass_index, index, function_name);
                        if skipped_functions.contains(&skip_key) {
                            continue;
                        }
                        let (pass_changed, skip) = run_function_pass(
                            pass.as_mut(),
                            module,
                            index,
                            PassRunContext {
                                iteration: iteration + 1,
                                verify,
                                report: &mut report,
                                observer: &mut observer,
                                counter,
                            },
                        )?;
                        changed |= pass_changed;
                        if skip {
                            skipped_functions.insert(skip_key);
                        }
                    }
                }
                for (pass_index, pass) in self.module_passes.iter_mut().enumerate() {
                    if skipped_module_passes.contains(&pass_index) {
                        continue;
                    }
                    let (pass_changed, skip) = run_module_pass(
                        pass.as_mut(),
                        module,
                        iteration + 1,
                        verify,
                        &mut report,
                        &mut observer,
                        counter,
                    )?;
                    changed |= pass_changed;
                    if skip {
                        skipped_module_passes.insert(pass_index);
                    }
                }
                report.iterations = iteration + 1;
                if !changed {
                    return Ok(report);
                }
            }
            report.hit_iteration_limit = self.options.max_iterations != 0;
            Ok(report)
        })();

        if result.is_err() {
            *module = snapshot;
        }
        result
    }
}
