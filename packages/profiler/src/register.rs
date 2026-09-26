//! Typed registration and Lisp adapters for the profiler extension.

use std::sync::Arc;

use ncl_object::{
    Arity as ObjectArity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError,
    Package, Parameter, ParameterType, Runtime, ThreadContext, Word, car, cdr, make_cons,
    make_string, set_symbol_macro,
};

use crate::ReportFormat;

const NO_PARAMETERS: &[Parameter] = &[];
const BODY_PARAMETERS: &[Parameter] = &[Parameter {
    name: BuiltinName::new("BODY"),
    ty: ParameterType::Any,
}];
const REPORT_PARAMETERS: &[Parameter] = &[Parameter {
    name: BuiltinName::new("FORMAT"),
    ty: ParameterType::Fixnum,
}];

/// Register the profiler package and all callable Lisp-facing entries.
///
/// # Errors
/// Returns an object-layer error if package, symbol, or builtin registration
/// fails.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    runtime.set_extension(Arc::new(crate::ProfileSession::new()));
    let mut context = ThreadContext::new();
    context.register(runtime)?;
    register_function(runtime, &mut context, "PROFILE-START", profile_start)?;
    register_function(runtime, &mut context, "PROFILE-STOP", profile_stop)?;
    register_report(runtime, &mut context)?;
    register_macro(runtime, &mut context, "WITH-PROFILING", with_profiling)
}

fn register_report(runtime: &Runtime, context: &mut ThreadContext) -> Result<(), ObjectError> {
    runtime.register_builtin(
        context,
        BuiltinIdentifier::new(
            BuiltinPackage::NclProfiler,
            BuiltinName::new("PROFILE-REPORT"),
        ),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::with_optional(NO_PARAMETERS, REPORT_PARAMETERS),
                convention: BuiltinConvention::Adapted,
            },
            profile_report,
            identity_args,
        ),
    )?;
    Ok(())
}

fn identity_args(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    for value in args.as_slice() {
        if !matches!(value.as_fixnum(), Some(0..=3)) {
            return Err(ObjectError::TypeError);
        }
    }
    Ok(args.as_slice().to_vec())
}

const fn descriptor(parameters: &'static [Parameter], arity: u8) -> Builtin {
    Builtin {
        lambda_list: LambdaList::fixed(parameters),
        convention: BuiltinConvention::Direct(ObjectArity::exact(arity)),
    }
}

fn register_function(
    runtime: &Runtime,
    context: &mut ThreadContext,
    name: &'static str,
    function: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    runtime.register_builtin(
        context,
        BuiltinIdentifier::new(BuiltinPackage::NclProfiler, BuiltinName::new(name)),
        BuiltinImplementation::direct(descriptor(NO_PARAMETERS, 0), function),
    )?;
    Ok(())
}

fn register_macro(
    runtime: &Runtime,
    context: &mut ThreadContext,
    name: &'static str,
    function: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    let identifier = BuiltinIdentifier::new(BuiltinPackage::NclProfiler, BuiltinName::new(name));
    runtime.register_builtin(
        context,
        identifier,
        BuiltinImplementation::direct(descriptor(BODY_PARAMETERS, 1), function),
    )?;
    let package = runtime.ensure_package(context, BuiltinPackage::NclProfiler.as_str())?;
    let (symbol, _) = Package::from_word(package).intern(context, runtime, name)?;
    set_symbol_macro(context, symbol, true)
}

fn session(runtime: &Runtime) -> Result<Arc<crate::ProfileSession>, ObjectError> {
    runtime
        .get_extension::<crate::ProfileSession>()
        .ok_or(ObjectError::Layout)
}

fn profile_start(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let session = session(runtime)?;
    session.start().map_err(|_| ObjectError::Layout)?;
    sample_boundary(&session, ctx)?;
    Ok(Word::NIL)
}

fn profile_stop(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let session = session(runtime)?;
    sample_boundary(&session, ctx)?;
    let _snapshot = session.stop().map_err(|_| ObjectError::Layout)?;
    Ok(Word::NIL)
}

fn sample_boundary(
    session: &crate::ProfileSession,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    match session.sample_current_thread(ctx.thread_mut()) {
        Ok(()) | Err(crate::SessionError::Sampling(crate::SampleError::EmptyStack)) => Ok(()),
        Err(_) => Err(ObjectError::Layout),
    }
}

fn profile_report(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let format = report_format(args)?;
    let report = session(runtime)?
        .report(format)
        .map_err(|_| ObjectError::Layout)?;
    make_string(ctx, runtime, &report.as_str().chars().collect::<Vec<_>>())
}

fn report_format(args: &BuiltinArgs<'_>) -> Result<ReportFormat, ObjectError> {
    let Some(value) = args.get(0) else {
        return Ok(ReportFormat::Flat);
    };
    match value.as_fixnum() {
        Some(0) => Ok(ReportFormat::Flat),
        Some(1) => Ok(ReportFormat::Cumulative),
        Some(2) => Ok(ReportFormat::Callgraph),
        Some(3) => Ok(ReportFormat::Folded),
        _ => Err(ObjectError::TypeError),
    }
}

fn with_profiling(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let form = args.required(0)?;
    let body = cdr(ctx, form)?;
    let start = call0(ctx, runtime, "PROFILE-START")?;
    let stop = call0(ctx, runtime, "PROFILE-STOP")?;
    let report = call0(ctx, runtime, "PROFILE-REPORT")?;
    let mut protected = vec![start];
    let mut cursor = body;
    while cursor != Word::NIL {
        protected.push(car(ctx, cursor)?);
        cursor = cdr(ctx, cursor)?;
    }
    let protected_body = list(ctx, runtime, "PROGN", &protected)?;
    let cleanup = list(ctx, runtime, "PROGN", &[stop, report])?;
    list(ctx, runtime, "UNWIND-PROTECT", &[protected_body, cleanup])
}

fn call0(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Result<Word, ObjectError> {
    list(ctx, runtime, name, &[])
}

fn list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    let package_name = match name {
        "PROGN" | "UNWIND-PROTECT" => BuiltinPackage::CommonLisp,
        _ => BuiltinPackage::NclProfiler,
    };
    let package = runtime.ensure_package(ctx, package_name.as_str())?;
    let (operator, _) = Package::from_word(package).intern(ctx, runtime, name)?;
    let mut result = Word::NIL;
    for argument in arguments.iter().rev().copied() {
        result = make_cons(ctx, runtime, argument, result)?;
    }
    make_cons(ctx, runtime, operator, result)
}

/// The profiler builtins exposed by the NCL extension package.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfilerBuiltin {
    /// Start a new profile session.
    Start,
    /// Stop the active profile session.
    Stop,
    /// Render the current profile as a string.
    Report,
    /// Expand a protected body into profiling forms.
    WithProfiling,
}

/// Typed argument cardinality for a builtin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arity {
    /// A fixed number of arguments.
    Exact(u8),
    /// A fixed minimum and maximum argument count.
    Range(u8, u8),
}

/// Typed result category for a builtin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReturnKind {
    /// The builtin returns the Lisp NIL value.
    Nil,
    /// The builtin returns a report string.
    Report,
}

/// Complete contract for one profiler builtin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuiltinContract {
    /// Builtin identity.
    pub builtin: ProfilerBuiltin,
    /// Accepted argument shape.
    pub arity: Arity,
    /// Result category.
    pub returns: ReturnKind,
}

/// The frozen profiler builtin contract.
pub const BUILTIN_CONTRACTS: &[BuiltinContract] = &[
    BuiltinContract {
        builtin: ProfilerBuiltin::Start,
        arity: Arity::Exact(0),
        returns: ReturnKind::Nil,
    },
    BuiltinContract {
        builtin: ProfilerBuiltin::Stop,
        arity: Arity::Exact(0),
        returns: ReturnKind::Nil,
    },
    BuiltinContract {
        builtin: ProfilerBuiltin::Report,
        arity: Arity::Range(0, 1),
        returns: ReturnKind::Report,
    },
    BuiltinContract {
        builtin: ProfilerBuiltin::WithProfiling,
        arity: Arity::Exact(1),
        returns: ReturnKind::Nil,
    },
];

/// Return the frozen builtin contract.
#[must_use]
pub const fn builtin_contracts() -> &'static [BuiltinContract] {
    BUILTIN_CONTRACTS
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "registration tests assert setup failures"
    )]

    use ncl_object::{
        FunctionObject, Package, Runtime, ThreadContext, Word, car, make_cons, string_length,
    };

    use super::register;

    fn function(runtime: &Runtime, context: &mut ThreadContext, name: &str) -> FunctionObject {
        runtime
            .function(context, "NCL-PROFILER", name)
            .ok_or(())
            .and_then(|word| FunctionObject::try_from(word).map_err(|_| ()))
            .expect("registered profiler function")
    }

    #[test]
    fn lisp_builtins_share_a_runtime_owned_session() {
        let runtime = Runtime::new().expect("runtime");
        register(&runtime).expect("registration");
        let mut context = ThreadContext::new();
        context.register(&runtime).expect("context");
        let start = function(&runtime, &mut context, "PROFILE-START");
        let stop = function(&runtime, &mut context, "PROFILE-STOP");
        let report = function(&runtime, &mut context, "PROFILE-REPORT");

        assert!(runtime.call_builtin(&mut context, report, &[]).is_err());
        assert!(runtime.call_builtin(&mut context, stop, &[]).is_err());
        assert_eq!(
            runtime.call_builtin(&mut context, start, &[]),
            Ok(Word::NIL)
        );
        let report_value = runtime.call_builtin(&mut context, report, &[]);
        assert!(report_value.is_ok());
        if let Ok(value) = report_value {
            assert_eq!(string_length(&context, value), Ok(0));
        }
        assert_eq!(runtime.call_builtin(&mut context, stop, &[]), Ok(Word::NIL));
        assert!(runtime.call_builtin(&mut context, stop, &[]).is_err());
    }

    #[test]
    fn with_profiling_expands_to_unwind_protect() {
        let runtime = Runtime::new().expect("runtime");
        register(&runtime).expect("registration");
        let mut context = ThreadContext::new();
        context.register(&runtime).expect("context");
        let package = runtime
            .ensure_package(&mut context, "NCL-PROFILER")
            .expect("package");
        let (macro_name, _) = Package::from_word(package)
            .intern(&mut context, &runtime, "WITH-PROFILING")
            .expect("macro symbol");
        let body = make_cons(&mut context, &runtime, Word::fixnum(42), Word::NIL).expect("body");
        let form = make_cons(&mut context, &runtime, macro_name, body).expect("form");
        let macro_function = function(&runtime, &mut context, "WITH-PROFILING");
        let expansion = runtime
            .call_builtin(&mut context, macro_function, &[form])
            .expect("expansion");
        let operator = car(&mut context, expansion).expect("operator");
        let (unwind, _) = Package::from_word(
            runtime
                .ensure_package(&mut context, "COMMON-LISP")
                .expect("common lisp"),
        )
        .intern(&mut context, &runtime, "UNWIND-PROTECT")
        .expect("unwind symbol");
        assert_eq!(operator, unwind);
    }

    #[test]
    fn report_accepts_each_typed_format_selector() {
        let runtime = Runtime::new().expect("runtime");
        register(&runtime).expect("registration");
        let mut context = ThreadContext::new();
        context.register(&runtime).expect("context");
        let report = function(&runtime, &mut context, "PROFILE-REPORT");
        assert!(runtime.call_builtin(&mut context, report, &[]).is_err());
        let start = function(&runtime, &mut context, "PROFILE-START");
        assert_eq!(
            runtime.call_builtin(&mut context, start, &[]),
            Ok(Word::NIL)
        );
        for selector in 0..4 {
            assert!(
                runtime
                    .call_builtin(&mut context, report, &[Word::fixnum(selector)])
                    .is_ok()
            );
        }
        assert!(
            runtime
                .call_builtin(&mut context, report, &[Word::fixnum(4)])
                .is_err()
        );
    }
}
