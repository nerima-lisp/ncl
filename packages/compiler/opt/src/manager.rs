use super::{
    DeadCodeElimination, FunctionPass, Module, ModulePass, PassError, PassEvent, PassManager,
    PassManagerOptions, PassReport, PassStat, PassVerification, Sccp,
};
use std::time::Instant;

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

    /// Adds the GC-safe default subset of optimization passes.
    ///
    /// Inlining can copy raw stores, and GVN can reuse address values across safepoints,
    /// so neither pass belongs in the runtime's default pipeline yet.
    pub fn add_default_optimization_pipeline(&mut self) {
        self.add_function_pass(Sccp);
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

    /// Runs all passes and reports every invocation to `observer`.
    ///
    /// The observer is called after verification for successful invocations and on the
    /// failure path for pass and verification errors.
    ///
    /// # Errors
    ///
    /// Returns a pass error or a post-pass verification failure.
    pub fn run_with_observer(
        &mut self,
        module: &mut Module,
        mut observer: impl FnMut(&PassEvent),
    ) -> Result<PassReport, PassError> {
        let mut report = PassReport::default();
        let verify = self.options.verify_after_each_pass;
        for iteration in 0..self.options.max_iterations {
            let mut changed = false;
            for pass in &mut self.function_passes {
                for index in 0..module.functions.len() {
                    changed |= run_function_pass(
                        pass.as_mut(),
                        module,
                        index,
                        iteration + 1,
                        verify,
                        &mut report,
                        &mut observer,
                    )?;
                }
            }
            for pass in &mut self.module_passes {
                changed |= run_module_pass(
                    pass.as_mut(),
                    module,
                    iteration + 1,
                    verify,
                    &mut report,
                    &mut observer,
                )?;
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

fn run_function_pass(
    pass: &mut dyn FunctionPass,
    module: &mut Module,
    index: usize,
    iteration: usize,
    verify: bool,
    report: &mut PassReport,
    observer: &mut impl FnMut(&PassEvent),
) -> Result<bool, PassError> {
    let pass_name = pass.name().to_owned();
    let Some(function_name) = module
        .functions
        .get(index)
        .map(|function| function.name.clone())
    else {
        return Err(PassError::new(&pass_name, "function index out of bounds"));
    };
    let snapshot = module.clone();
    let started = Instant::now();
    let Some(function) = module.functions.get_mut(index) else {
        return Err(PassError::new(&pass_name, "function index out of bounds"));
    };
    let result = pass.run(function, &snapshot);
    let elapsed = started.elapsed();
    let did_change = match result {
        Ok(changed) => changed,
        Err(error) => {
            let error = PassError::new(&pass_name, error.message);
            observer(&PassEvent {
                iteration,
                function: Some(function_name),
                pass: pass_name,
                changed: None,
                verification: PassVerification::NotRun,
                elapsed,
                error: Some(error.message.clone()),
            });
            return Err(error);
        }
    };
    report.stats.push(PassStat {
        pass: pass_name.clone(),
        changed: did_change,
        elapsed,
    });
    verify_and_notify(
        module,
        verify,
        PassInvocation {
            iteration,
            function: Some(function_name),
            pass: pass_name,
            changed: did_change,
            elapsed,
        },
        observer,
    )
}

fn run_module_pass(
    pass: &mut dyn ModulePass,
    module: &mut Module,
    iteration: usize,
    verify: bool,
    report: &mut PassReport,
    observer: &mut impl FnMut(&PassEvent),
) -> Result<bool, PassError> {
    let pass_name = pass.name().to_owned();
    let started = Instant::now();
    let result = pass.run(module);
    let elapsed = started.elapsed();
    let did_change = match result {
        Ok(changed) => changed,
        Err(error) => {
            let error = PassError::new(&pass_name, error.message);
            observer(&PassEvent {
                iteration,
                function: None,
                pass: pass_name,
                changed: None,
                verification: PassVerification::NotRun,
                elapsed,
                error: Some(error.message.clone()),
            });
            return Err(error);
        }
    };
    report.stats.push(PassStat {
        pass: pass_name.clone(),
        changed: did_change,
        elapsed,
    });
    verify_and_notify(
        module,
        verify,
        PassInvocation {
            iteration,
            function: None,
            pass: pass_name,
            changed: did_change,
            elapsed,
        },
        observer,
    )
}

struct PassInvocation {
    iteration: usize,
    function: Option<String>,
    pass: String,
    changed: bool,
    elapsed: std::time::Duration,
}

fn verify_and_notify(
    module: &Module,
    verify: bool,
    invocation: PassInvocation,
    observer: &mut impl FnMut(&PassEvent),
) -> Result<bool, PassError> {
    match verify_after(verify, &invocation.pass, module) {
        Ok(()) => observer(&PassEvent {
            iteration: invocation.iteration,
            function: invocation.function,
            pass: invocation.pass,
            changed: Some(invocation.changed),
            verification: if verify {
                PassVerification::Passed
            } else {
                PassVerification::Skipped
            },
            elapsed: invocation.elapsed,
            error: None,
        }),
        Err(error) => {
            observer(&PassEvent {
                iteration: invocation.iteration,
                function: invocation.function,
                pass: invocation.pass,
                changed: Some(invocation.changed),
                verification: PassVerification::Failed(error.message.clone()),
                elapsed: invocation.elapsed,
                error: Some(error.message.clone()),
            });
            return Err(error);
        }
    }
    Ok(invocation.changed)
}

fn verify_after(enabled: bool, pass: &str, module: &Module) -> Result<(), PassError> {
    if enabled {
        module
            .verify()
            .map_err(|message| PassError::new(pass, message))?;
    }
    Ok(())
}
