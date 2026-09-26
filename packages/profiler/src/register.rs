//! Typed contract and symbol registration for the profiler's Lisp-facing builtins.

use ncl_object::{ObjectError, Runtime, ThreadContext, Word};

const FUNCTIONS: &[&str] = &["PROFILE-START", "PROFILE-STOP", "PROFILE-REPORT"];
const MACROS: &[&str] = &["WITH-PROFILING"];

/// Register the profiler package symbols in the standard-library order.
///
/// # Errors
///
/// Returns an object-layer error if package or symbol registration fails.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut context = ThreadContext::new();
    context.register(runtime)?;
    for name in FUNCTIONS {
        runtime.define_function(&mut context, "NCL-PROFILER", name, Word::UNBOUND)?;
    }
    for name in MACROS {
        runtime.define_function(&mut context, "NCL-PROFILER", name, Word::UNBOUND)?;
    }
    Ok(())
}

/// The profiler builtins exposed by the NCL extension package.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfilerBuiltin {
    /// Begin sampling and discard the previous session.
    Start,
    /// Stop sampling and retain the collected profile.
    Stop,
    /// Render the retained profile using a report format.
    Report,
    /// Profile a protected Lisp body.
    WithProfiling,
}

/// Typed argument cardinality for a builtin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arity {
    /// No arguments.
    Exact(u8),
    /// One optional argument.
    Optional(u8),
}

/// Typed result category for a builtin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReturnKind {
    /// No meaningful value is returned.
    Nil,
    /// A report object is returned.
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
        arity: Arity::Optional(1),
        returns: ReturnKind::Nil,
    },
    BuiltinContract {
        builtin: ProfilerBuiltin::Stop,
        arity: Arity::Exact(0),
        returns: ReturnKind::Nil,
    },
    BuiltinContract {
        builtin: ProfilerBuiltin::Report,
        arity: Arity::Optional(1),
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
