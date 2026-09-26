//! Coverage checks that compare a live runtime against the ownership table.

use ncl_object::{
    ObjectError, Package, Runtime, ThreadContext, Word, make_string, pop_root, push_root,
    symbol_function, symbol_is_constant, symbol_is_macro, symbol_is_special,
};

use crate::table::{Kind, Row};

/// Implementation phase whose rows the gate requires a crate to register.
const PHASE_ONE: u8 = 1;

/// One owned symbol that a runtime did not register.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Missing {
    /// Owning package name.
    pub package: String,
    /// Symbol name.
    pub symbol: String,
    /// Kinds that failed for the symbol.
    pub kind: Vec<Kind>,
    /// Why the symbol counts as missing.
    pub reason: &'static str,
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kinds = self
            .kind
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("+");
        write!(
            f,
            "{}::{} ({}): {}",
            self.package, self.symbol, kinds, self.reason
        )
    }
}

/// Failure of an ownership-table parse or a coverage check.
#[derive(Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum OwnershipError {
    /// The table has no rows for the requested crate and phase.
    NoRows {
        /// Crate name that has no Phase 1 rows.
        crate_name: String,
    },
    /// A table row could not be parsed.
    BadRow {
        /// One-based line number in `symbols.tsv`.
        line: usize,
        /// Why the line was rejected.
        reason: &'static str,
    },
    /// An object-layer lookup failed while checking coverage.
    Object(ObjectError),
    /// One or more owned symbols are not registered.
    Missing(Vec<Missing>),
}

impl OwnershipError {
    /// Build a [`Self::BadRow`] failure.
    pub(crate) const fn bad_row(line: usize, reason: &'static str) -> Self {
        Self::BadRow { line, reason }
    }
}

impl From<ObjectError> for OwnershipError {
    fn from(error: ObjectError) -> Self {
        Self::Object(error)
    }
}

impl std::fmt::Display for OwnershipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRows { crate_name } => write!(f, "no Phase 1 rows for crate {crate_name}"),
            Self::BadRow { line, reason } => write!(f, "symbols.tsv line {line}: {reason}"),
            Self::Object(error) => write!(f, "object error: {error}"),
            Self::Missing(missing) => {
                let report = missing
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n");
                f.write_str(&report)
            }
        }
    }
}

impl std::fmt::Debug for OwnershipError {
    /// Render the same report as [`Display`](std::fmt::Display) so that
    /// `Result::unwrap` prints one line per missing symbol.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for OwnershipError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Object(error) => Some(error),
            _ => None,
        }
    }
}

/// Check a crate against table text supplied by its ownership test.
///
/// # Errors
///
/// Returns [`OwnershipError::NoRows`] when the crate has no Phase 1 rows, so an
/// empty selection cannot pass vacuously, and [`OwnershipError::Missing`]
/// listing every unregistered symbol. A failed object-layer lookup or malformed
/// table is returned as the corresponding error variant.
pub fn assert_crate_coverage_from_table(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    table: &str,
    crate_name: &str,
) -> Result<(), OwnershipError> {
    let rows = crate::table::rows_for_crate_from_str(table, crate_name, PHASE_ONE)?;
    if rows.is_empty() {
        return Err(OwnershipError::NoRows {
            crate_name: crate_name.to_owned(),
        });
    }
    let mut missing = Vec::new();
    for row in &rows {
        check_row(runtime, ctx, row, &mut missing)?;
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(OwnershipError::Missing(missing))
    }
}

/// Check function-cell bindings against caller-supplied ownership table text.
///
/// # Errors
///
/// Returns [`OwnershipError::NoRows`] when the crate has no Phase 1 rows and
/// [`OwnershipError::Missing`] for an unregistered or unbound function.
pub fn assert_crate_function_bindings_from_table(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    table: &str,
    crate_name: &str,
) -> Result<(), OwnershipError> {
    let rows = crate::table::rows_for_crate_from_str(table, crate_name, PHASE_ONE)?;
    if rows.is_empty() {
        return Err(OwnershipError::NoRows {
            crate_name: crate_name.to_owned(),
        });
    }
    check_function_bindings(runtime, ctx, &rows)
}

fn check_function_bindings(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    rows: &[Row],
) -> Result<(), OwnershipError> {
    let mut missing = Vec::new();
    for row in rows.iter().filter(|row| row.kind.contains(&Kind::Function)) {
        let Some(package_word) = runtime.find_package(ctx, &row.package) else {
            missing.push(single_kind(row, Kind::Function, "package not found"));
            continue;
        };
        let mut name = make_string(ctx, runtime, &row.symbol.chars().collect::<Vec<_>>())?;
        let mut package = package_word;
        let name_token = push_root(ctx, &mut name);
        let package_token = push_root(ctx, &mut package);
        let found = Package::from_word(package).find_symbol(ctx, name);
        let _ = pop_root(ctx, package_token);
        let _ = pop_root(ctx, name_token);
        let Some((mut symbol, _)) = found? else {
            missing.push(single_kind(row, Kind::Function, "symbol not interned"));
            continue;
        };
        let symbol_token = push_root(ctx, &mut symbol);
        let registered = runtime
            .function(ctx, &row.package, &row.symbol)
            .is_some_and(|value| value != Word::UNBOUND);
        let bound = symbol_function(ctx, symbol)? != Word::UNBOUND;
        let _ = pop_root(ctx, symbol_token);
        if !registered {
            missing.push(single_kind(row, Kind::Function, "function not registered"));
        } else if !bound {
            missing.push(single_kind(
                row,
                Kind::Function,
                "symbol function cell unbound",
            ));
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(OwnershipError::Missing(missing))
    }
}

/// Check one table row, appending every failure to `missing`.
fn check_row(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    row: &Row,
    missing: &mut Vec<Missing>,
) -> Result<(), OwnershipError> {
    let mut name = make_string(ctx, runtime, &row.symbol.chars().collect::<Vec<_>>())?;
    let Some(package_word) = runtime.find_package(ctx, &row.package) else {
        missing.push(row_missing(row, "package not found"));
        return Ok(());
    };
    let mut package = package_word;
    let name_token = push_root(ctx, &mut name);
    let package_token = push_root(ctx, &mut package);
    let found = Package::from_word(package).find_symbol(ctx, name);
    let _ = pop_root(ctx, package_token);
    let _ = pop_root(ctx, name_token);
    let Some((mut symbol, _status)) = found? else {
        missing.push(row_missing(row, "symbol not interned"));
        return Ok(());
    };
    let symbol_token = push_root(ctx, &mut symbol);
    let result = check_kinds(runtime, ctx, row, symbol, missing);
    let _ = pop_root(ctx, symbol_token);
    result
}

/// Check each kind recorded for an interned symbol.
fn check_kinds(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    row: &Row,
    symbol: Word,
    missing: &mut Vec<Missing>,
) -> Result<(), OwnershipError> {
    for kind in &row.kind {
        match kind {
            Kind::Function => {
                if runtime.function(ctx, &row.package, &row.symbol).is_none() {
                    missing.push(single_kind(row, *kind, "function not registered"));
                }
            }
            Kind::Class | Kind::Condition => {
                if runtime.class(ctx, &row.symbol).is_none() {
                    missing.push(single_kind(row, *kind, "class not registered"));
                }
            }
            Kind::Macro => {
                if !symbol_is_macro(ctx, symbol)? {
                    missing.push(single_kind(row, *kind, "macro bit not set"));
                }
            }
            Kind::Variable => {
                if !symbol_is_special(ctx, symbol)? {
                    missing.push(single_kind(row, *kind, "special bit not set"));
                }
            }
            Kind::Constant => {
                if !symbol_is_constant(ctx, symbol)? {
                    missing.push(single_kind(row, *kind, "constant bit not set"));
                }
            }
            Kind::Other | Kind::SpecialOperator | Kind::Type => {}
        }
    }
    Ok(())
}

/// Build a [`Missing`] that carries every kind of a row.
fn row_missing(row: &Row, reason: &'static str) -> Missing {
    Missing {
        package: row.package.clone(),
        symbol: row.symbol.clone(),
        kind: row.kind.clone(),
        reason,
    }
}

/// Build a [`Missing`] that carries a single kind of a row.
fn single_kind(row: &Row, kind: Kind, reason: &'static str) -> Missing {
    Missing {
        package: row.package.clone(),
        symbol: row.symbol.clone(),
        kind: vec![kind],
        reason,
    }
}
