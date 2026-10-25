use super::{
    DeadCodeElimination, FunctionPass, GlobalValueNumbering, InlineDirectCalls, Module, ModulePass,
    PassError, PassEvent, PassManager, PassManagerOptions, PassReport, PassStat, PassVerification,
    Sccp, TypeEffect,
};
use ncl_ir::{Function, VerifyError};
use std::collections::HashSet;
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
        mut observer: impl FnMut(&PassEvent),
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
                            iteration + 1,
                            verify,
                            &mut report,
                            &mut observer,
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

fn address_liveness_errors(function: &Function) -> Vec<VerifyError> {
    ncl_ir::verify(function)
        .err()
        .unwrap_or_default()
        .into_iter()
        .filter(|error| matches!(error, VerifyError::AddressLiveAcross { .. }))
        .collect()
}

fn added_provenance_errors(previous: &[VerifyError], function: &Function) -> Vec<VerifyError> {
    let mut unmatched = previous.to_vec();
    let mut added = Vec::new();
    for error in address_liveness_errors(function) {
        if let Some(index) = unmatched.iter().position(|existing| existing == &error) {
            unmatched.remove(index);
        } else {
            added.push(error);
        }
    }
    added
}

fn new_module_provenance_errors(before: &Module, after: &Module) -> Vec<(String, VerifyError)> {
    let mut added = Vec::new();
    for function in &after.functions {
        let previous = before
            .functions
            .iter()
            .find(|candidate| candidate.id == function.id)
            .map_or_else(Vec::new, address_liveness_errors);
        added.extend(
            added_provenance_errors(&previous, function)
                .into_iter()
                .map(|error| (function.name.clone(), error)),
        );
    }
    added
}

fn run_function_pass(
    pass: &mut dyn FunctionPass,
    module: &mut Module,
    index: usize,
    iteration: usize,
    verify: bool,
    report: &mut PassReport,
    observer: &mut impl FnMut(&PassEvent),
) -> Result<(bool, bool), PassError> {
    let pass_name = pass.name().to_owned();
    let Some(function_name) = module
        .functions
        .get(index)
        .map(|function| function.name.clone())
    else {
        return Err(PassError::new(&pass_name, "function index out of bounds"));
    };
    let snapshot = module.clone();
    let Some(snapshot_function) = snapshot.functions.get(index).cloned() else {
        return Err(PassError::new(&pass_name, "function index out of bounds"));
    };
    let started = Instant::now();
    let Some(function) = module.functions.get_mut(index) else {
        return Err(PassError::new(&pass_name, "function index out of bounds"));
    };
    let result = pass.run(function, &snapshot);
    let elapsed = started.elapsed();
    let Some(updated_function) = module.functions.get(index) else {
        return Err(PassError::new(&pass_name, "function index out of bounds"));
    };
    let original_address_errors = address_liveness_errors(&snapshot_function);
    let new_errors = added_provenance_errors(&original_address_errors, updated_function);
    let did_change = match result {
        Ok(changed) => changed,
        Err(error) => {
            if !new_errors.is_empty() {
                return rollback_function_pass(
                    module,
                    index,
                    snapshot_function,
                    PassInvocation {
                        iteration,
                        function: Some(function_name),
                        pass: pass_name,
                        changed: false,
                        elapsed,
                    },
                    report,
                    observer,
                    &format!("{new_errors:?}; pass error: {}", error.message),
                );
            }
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
    if !new_errors.is_empty() {
        return rollback_function_pass(
            module,
            index,
            snapshot_function,
            PassInvocation {
                iteration,
                function: Some(function_name),
                pass: pass_name,
                changed: false,
                elapsed,
            },
            report,
            observer,
            &format!("{new_errors:?}"),
        );
    }
    report.stats.push(PassStat {
        pass: pass_name.clone(),
        changed: did_change,
        elapsed,
    });
    let changed = verify_and_notify(
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
    )?;
    Ok((changed, false))
}

fn rollback_function_pass(
    module: &mut Module,
    index: usize,
    snapshot_function: Function,
    invocation: PassInvocation,
    report: &mut PassReport,
    observer: &mut impl FnMut(&PassEvent),
    detail: &str,
) -> Result<(bool, bool), PassError> {
    let Some(function) = module.functions.get_mut(index) else {
        return Err(PassError::new(
            &invocation.pass,
            "function index out of bounds",
        ));
    };
    *function = snapshot_function;
    let message = format!(
        "provenance verification rolled back {}: {detail}",
        invocation.pass
    );
    report.stats.push(PassStat {
        pass: invocation.pass.clone(),
        changed: false,
        elapsed: invocation.elapsed,
    });
    observer(&PassEvent {
        iteration: invocation.iteration,
        function: invocation.function,
        pass: invocation.pass,
        changed: Some(invocation.changed),
        verification: PassVerification::RolledBack(message.clone()),
        elapsed: invocation.elapsed,
        error: Some(message),
    });
    Ok((false, true))
}

fn run_module_pass(
    pass: &mut dyn ModulePass,
    module: &mut Module,
    iteration: usize,
    verify: bool,
    report: &mut PassReport,
    observer: &mut impl FnMut(&PassEvent),
) -> Result<(bool, bool), PassError> {
    let pass_name = pass.name().to_owned();
    let snapshot = module.clone();
    let started = Instant::now();
    let result = pass.run(module);
    let elapsed = started.elapsed();
    let did_change = match result {
        Ok(changed) => changed,
        Err(error) => {
            let new_errors = new_module_provenance_errors(&snapshot, module);
            if !new_errors.is_empty() {
                *module = snapshot;
                let message = format!(
                    "provenance verification rolled back {pass_name}: {new_errors:?}; pass error: {}",
                    error.message
                );
                report.stats.push(PassStat {
                    pass: pass_name.clone(),
                    changed: false,
                    elapsed,
                });
                observer(&PassEvent {
                    iteration,
                    function: None,
                    pass: pass_name,
                    changed: Some(false),
                    verification: PassVerification::RolledBack(message.clone()),
                    elapsed,
                    error: Some(message),
                });
                return Ok((false, true));
            }
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
    let new_errors = new_module_provenance_errors(&snapshot, module);
    if !new_errors.is_empty() {
        *module = snapshot;
        let message = format!("provenance verification rolled back {pass_name}: {new_errors:?}");
        report.stats.push(PassStat {
            pass: pass_name.clone(),
            changed: false,
            elapsed,
        });
        observer(&PassEvent {
            iteration,
            function: None,
            pass: pass_name,
            changed: Some(false),
            verification: PassVerification::RolledBack(message.clone()),
            elapsed,
            error: Some(message),
        });
        return Ok((false, true));
    }
    report.stats.push(PassStat {
        pass: pass_name.clone(),
        changed: did_change,
        elapsed,
    });
    let changed = verify_and_notify(
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
    )?;
    Ok((changed, false))
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
