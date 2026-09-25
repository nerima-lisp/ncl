//! Registration of the symbols owned by `ncl-image`.

use ncl_object::{ObjectError, Package, Runtime, ThreadContext, Word, set_symbol_special};

/// Kind of binding a row requires for the ownership gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    /// A special variable: the symbol's special bit must be set.
    Variable,
    /// A function name: the runtime must have a registered function.
    Function,
    /// A class and a function name.
    ClassAndFunction,
}

/// One owned symbol.
#[derive(Clone, Copy, Debug)]
struct Row {
    package: &'static str,
    name: &'static str,
    kind: Kind,
}

/// The Phase 1 rows formerly assigned to `ncl-image`.
const ROWS: &[Row] = &[
    Row {
        package: "SB-EXT",
        name: "*POSIX-ARGV*",
        kind: Kind::Variable,
    },
    Row {
        package: "SB-EXT",
        name: "EXIT",
        kind: Kind::ClassAndFunction,
    },
    Row {
        package: "SB-EXT",
        name: "QUIT",
        kind: Kind::Function,
    },
    Row {
        package: "SB-EXT",
        name: "SAVE-LISP-AND-DIE",
        kind: Kind::Function,
    },
    Row {
        package: "SB-SYS",
        name: "OS-COLD-INIT-OR-REINIT",
        kind: Kind::Function,
    },
    Row {
        package: "SB-SYS",
        name: "OS-DEINIT",
        kind: Kind::Function,
    },
    Row {
        package: "SB-SYS",
        name: "OS-EXIT",
        kind: Kind::Function,
    },
];

/// Register every symbol owned by `ncl-image`.
///
/// This is the per-crate registration entry point that `ncl-stdlib` calls in
/// dependency order. It creates an internal context, so callers pass only the
/// shared runtime.
///
/// # Errors
///
/// Returns an object-layer error when package creation, interning, or
/// registration fails.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for row in ROWS {
        let package = runtime.ensure_package(&mut ctx, row.package)?;
        let (symbol, _status) = Package::from(package).intern(&mut ctx, runtime, row.name)?;
        match row.kind {
            Kind::Variable => set_symbol_special(&mut ctx, symbol, true)?,
            Kind::Function => {
                runtime.define_function(&mut ctx, row.package, row.name, Word::UNBOUND)?;
            }
            Kind::ClassAndFunction => {
                runtime.define_function(&mut ctx, row.package, row.name, Word::UNBOUND)?;
                runtime.define_class(&mut ctx, row.name, Word::fixnum(1))?;
            }
        }
    }
    Ok(())
}
