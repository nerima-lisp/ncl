use super::allocation_counter::AllocationCounter;
use super::provenance::{
    added_provenance_errors, address_liveness_errors, new_module_provenance_errors,
};
use super::{
    FunctionPass, Module, ModulePass, PassError, PassEvent, PassReport, PassStat, PassVerification,
};
use ncl_ir::Function;
use std::time::Instant;

pub(super) trait PassObserver {
    const ENABLED: bool;

    fn observe(&mut self, event: &PassEvent);
}

impl<F: FnMut(&PassEvent)> PassObserver for F {
    const ENABLED: bool = true;

    fn observe(&mut self, event: &PassEvent) {
        self(event);
    }
}

pub(super) struct DisabledPassObserver;

impl PassObserver for DisabledPassObserver {
    const ENABLED: bool = false;

    fn observe(&mut self, _event: &PassEvent) {}
}

pub(super) struct PassRunContext<'a, C, O> {
    pub(super) iteration: usize,
    pub(super) verify: bool,
    pub(super) report: &'a mut PassReport,
    pub(super) observer: &'a mut O,
    pub(super) counter: &'a mut C,
}

struct FunctionPassOutcome {
    result: Result<bool, PassError>,
    elapsed: std::time::Duration,
}

fn run_function_pass_body(
    pass: &mut dyn FunctionPass,
    function: &mut Function,
    snapshot: &Module,
) -> FunctionPassOutcome {
    let started = Instant::now();
    let result = pass.run(function, snapshot);
    let elapsed = started.elapsed();
    FunctionPassOutcome { result, elapsed }
}

pub(super) fn run_function_pass<C, O>(
    pass: &mut dyn FunctionPass,
    module: &mut Module,
    index: usize,
    context: PassRunContext<'_, C, O>,
) -> Result<(bool, bool), PassError>
where
    C: AllocationCounter,
    O: PassObserver,
{
    let PassRunContext {
        iteration,
        verify,
        report,
        observer,
        counter,
    } = context;
    let pass_name = pass.name().to_owned();
    let snapshot = module.clone();
    let snapshot_function = snapshot
        .functions
        .get(index)
        .cloned()
        .ok_or_else(|| PassError::new(&pass_name, "function index out of bounds"))?;
    let function = module
        .functions
        .get_mut(index)
        .ok_or_else(|| PassError::new(&pass_name, "function index out of bounds"))?;
    let function_name = function.name.clone();
    let allocation_before = if C::ENABLED {
        Some(counter.before_pass())
    } else {
        None
    };
    let FunctionPassOutcome { result, elapsed } = run_function_pass_body(pass, function, &snapshot);
    let allocated_bytes = if C::ENABLED {
        counter.after_pass(allocation_before.flatten())
    } else {
        None
    };
    let original_address_errors = address_liveness_errors(&snapshot_function);
    let new_errors = added_provenance_errors(&original_address_errors, function);
    let did_change = match result {
        Ok(changed) => changed,
        Err(error) => {
            let invocation = PassInvocation::for_function(
                iteration,
                function_name,
                pass_name,
                false,
                elapsed,
                allocated_bytes,
            );
            if new_errors.is_empty() {
                return Err(report_function_pass_error(invocation, error, observer));
            }
            return Ok(rollback_function_pass(
                function,
                snapshot_function,
                invocation,
                report,
                observer,
                &format!("{new_errors:?}; pass error: {}", error.message),
            ));
        }
    };
    if !new_errors.is_empty() {
        return Ok(rollback_function_pass(
            function,
            snapshot_function,
            PassInvocation::for_function(
                iteration,
                function_name,
                pass_name,
                false,
                elapsed,
                allocated_bytes,
            ),
            report,
            observer,
            &format!("{new_errors:?}"),
        ));
    }
    report.stats.push(PassStat {
        pass: pass_name.clone(),
        changed: did_change,
        elapsed,
    });
    if O::ENABLED {
        let changed = verify_and_notify(
            module,
            verify,
            PassInvocation::for_function(
                iteration,
                function_name,
                pass_name,
                did_change,
                elapsed,
                allocated_bytes,
            ),
            observer,
        )?;
        return Ok((changed, false));
    }
    verify_after(verify, &pass_name, module)?;
    Ok((did_change, false))
}

fn rollback_function_pass<O: PassObserver>(
    function: &mut Function,
    snapshot_function: Function,
    invocation: PassInvocation,
    report: &mut PassReport,
    observer: &mut O,
    detail: &str,
) -> (bool, bool) {
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
    if O::ENABLED {
        observer.observe(&PassEvent {
            iteration: invocation.iteration,
            function: invocation.function,
            pass: invocation.pass,
            changed: Some(invocation.changed),
            verification: PassVerification::RolledBack(message.clone()),
            elapsed: invocation.elapsed,
            allocated_bytes: invocation.allocated_bytes,
            error: Some(message),
        });
    }
    (false, true)
}

pub(super) fn run_module_pass<C, O>(
    pass: &mut dyn ModulePass,
    module: &mut Module,
    iteration: usize,
    verify: bool,
    report: &mut PassReport,
    observer: &mut O,
    counter: &mut C,
) -> Result<(bool, bool), PassError>
where
    C: AllocationCounter,
    O: PassObserver,
{
    let pass_name = pass.name().to_owned();
    let snapshot = module.clone();
    let allocation_before = if C::ENABLED {
        counter.before_pass()
    } else {
        None
    };
    let started = Instant::now();
    let result = pass.run(module);
    let elapsed = started.elapsed();
    let allocated_bytes = if C::ENABLED {
        counter.after_pass(allocation_before)
    } else {
        None
    };
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
                if O::ENABLED {
                    observer.observe(&PassEvent {
                        iteration,
                        function: None,
                        pass: pass_name,
                        changed: Some(false),
                        verification: PassVerification::RolledBack(message.clone()),
                        elapsed,
                        allocated_bytes,
                        error: Some(message),
                    });
                }
                return Ok((false, true));
            }
            return Err(report_module_pass_error(
                pass_name,
                error,
                iteration,
                elapsed,
                allocated_bytes,
                observer,
            ));
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
        if O::ENABLED {
            observer.observe(&PassEvent {
                iteration,
                function: None,
                pass: pass_name,
                changed: Some(false),
                verification: PassVerification::RolledBack(message.clone()),
                elapsed,
                allocated_bytes,
                error: Some(message),
            });
        }
        return Ok((false, true));
    }
    report.stats.push(PassStat {
        pass: pass_name.clone(),
        changed: did_change,
        elapsed,
    });
    if O::ENABLED {
        let changed = verify_and_notify(
            module,
            verify,
            PassInvocation {
                iteration,
                function: None,
                pass: pass_name,
                changed: did_change,
                elapsed,
                allocated_bytes,
            },
            observer,
        )?;
        return Ok((changed, false));
    }
    verify_after(verify, &pass_name, module)?;
    Ok((did_change, false))
}

fn report_module_pass_error<O: PassObserver>(
    pass: String,
    error: PassError,
    iteration: usize,
    elapsed: std::time::Duration,
    allocated_bytes: Option<u128>,
    observer: &mut O,
) -> PassError {
    let error = PassError::new(&pass, error.message);
    if O::ENABLED {
        observer.observe(&PassEvent {
            iteration,
            function: None,
            pass,
            changed: None,
            verification: PassVerification::NotRun,
            elapsed,
            allocated_bytes,
            error: Some(error.message.clone()),
        });
    }
    error
}

struct PassInvocation {
    iteration: usize,
    function: Option<String>,
    pass: String,
    changed: bool,
    elapsed: std::time::Duration,
    allocated_bytes: Option<u128>,
}

impl PassInvocation {
    const fn for_function(
        iteration: usize,
        function: String,
        pass: String,
        changed: bool,
        elapsed: std::time::Duration,
        allocated_bytes: Option<u128>,
    ) -> Self {
        Self {
            iteration,
            function: Some(function),
            pass,
            changed,
            elapsed,
            allocated_bytes,
        }
    }
}

fn report_function_pass_error<O: PassObserver>(
    invocation: PassInvocation,
    error: PassError,
    observer: &mut O,
) -> PassError {
    let error = PassError::new(&invocation.pass, error.message);
    if O::ENABLED {
        observer.observe(&PassEvent {
            iteration: invocation.iteration,
            function: invocation.function,
            pass: invocation.pass,
            changed: None,
            verification: PassVerification::NotRun,
            elapsed: invocation.elapsed,
            allocated_bytes: invocation.allocated_bytes,
            error: Some(error.message.clone()),
        });
    }
    error
}

fn verify_and_notify<O: PassObserver>(
    module: &Module,
    verify: bool,
    invocation: PassInvocation,
    observer: &mut O,
) -> Result<bool, PassError> {
    match verify_after(verify, &invocation.pass, module) {
        Ok(()) => {
            if O::ENABLED {
                observer.observe(&PassEvent {
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
                    allocated_bytes: invocation.allocated_bytes,
                    error: None,
                });
            }
        }
        Err(error) => {
            if O::ENABLED {
                observer.observe(&PassEvent {
                    iteration: invocation.iteration,
                    function: invocation.function,
                    pass: invocation.pass,
                    changed: Some(invocation.changed),
                    verification: PassVerification::Failed(error.message.clone()),
                    elapsed: invocation.elapsed,
                    allocated_bytes: invocation.allocated_bytes,
                    error: Some(error.message.clone()),
                });
            }
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
