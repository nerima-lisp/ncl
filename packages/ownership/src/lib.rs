//! Strict, test-only checks for the symbol ownership table.
#![allow(missing_docs)]

use ncl_object::{
    make_string, symbol_flags, symbol_function, Package, Runtime, ThreadContext, Word,
};

const TABLE: &str = include_str!("../../../conformance/ownership/symbols.tsv");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Class,
    Condition,
    Constant,
    Function,
    Macro,
    Other,
    SpecialOperator,
    Type,
    Variable,
}

impl Kind {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "class" => Self::Class,
            "condition" => Self::Condition,
            "constant" => Self::Constant,
            "function" => Self::Function,
            "macro" => Self::Macro,
            "other" => Self::Other,
            "special-operator" => Self::SpecialOperator,
            "type" => Self::Type,
            "variable" => Self::Variable,
            _ => return None,
        })
    }
}
impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Class => "class",
            Self::Condition => "condition",
            Self::Constant => "constant",
            Self::Function => "function",
            Self::Macro => "macro",
            Self::Other => "other",
            Self::SpecialOperator => "special-operator",
            Self::Type => "type",
            Self::Variable => "variable",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Row {
    pub package: String,
    pub symbol: String,
    pub kind: Vec<Kind>,
    pub crate_name: String,
    pub phase: u8,
    pub direct_expansion: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Missing {
    pub package: String,
    pub symbol: String,
    pub kind: Kind,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnershipError {
    NoRows(String),
    BadRow(usize, &'static str),
    Object(ncl_object::ObjectError),
    Missing(Vec<Missing>),
}
impl From<ncl_object::ObjectError> for OwnershipError {
    fn from(error: ncl_object::ObjectError) -> Self {
        Self::Object(error)
    }
}
impl std::fmt::Display for OwnershipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRows(name) => write!(f, "no Phase 1 rows for crate {name}"),
            Self::BadRow(line, reason) => write!(f, "symbols.tsv line {line}: {reason}"),
            Self::Object(error) => error.fmt(f),
            Self::Missing(items) => {
                for (index, item) in items.iter().enumerate() {
                    if index != 0 {
                        f.write_str("\n")?;
                    }
                    write!(
                        f,
                        "{}::{} ({}): {}",
                        item.package, item.symbol, item.kind, item.reason
                    )?;
                }
                Ok(())
            }
        }
    }
}
impl std::error::Error for OwnershipError {}

fn parse(source: &str) -> Result<Vec<Row>, OwnershipError> {
    source
        .lines()
        .enumerate()
        .skip(1)
        .map(|(line, text)| {
            let fields = text.split('\t').collect::<Vec<_>>();
            if fields.len() != 7 {
                return Err(OwnershipError::BadRow(line + 1, "expected seven columns"));
            }
            let phase = fields[4]
                .parse()
                .map_err(|_| OwnershipError::BadRow(line + 1, "invalid phase"))?;
            let direct_expansion = match fields[5] {
                "yes" => true,
                "no" => false,
                _ => {
                    return Err(OwnershipError::BadRow(
                        line + 1,
                        "invalid direct-expansion flag",
                    ))
                }
            };
            let kind = fields[2]
                .split('+')
                .map(|part| {
                    Kind::parse(part).ok_or(OwnershipError::BadRow(line + 1, "invalid kind"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if fields[..5].iter().any(|field| field.is_empty()) || kind.is_empty() {
                return Err(OwnershipError::BadRow(line + 1, "empty field"));
            }
            Ok(Row {
                package: fields[0].into(),
                symbol: fields[1].into(),
                kind,
                crate_name: fields[3].into(),
                phase,
                direct_expansion,
            })
        })
        .collect()
}

/// # Errors
/// Returns a parse error when the checked-in table is malformed.
pub fn rows() -> Result<Vec<Row>, OwnershipError> {
    parse(TABLE)
}
/// # Errors
/// Returns a parse error when `source` is malformed.
pub fn rows_from_str(source: &str) -> Result<Vec<Row>, OwnershipError> {
    parse(source)
}
/// # Errors
/// Returns a parse error when the checked-in table is malformed.
pub fn rows_for_crate(crate_name: &str, phase: u8) -> Result<Vec<Row>, OwnershipError> {
    rows_from_str(TABLE).map(|rows| {
        rows.into_iter()
            .filter(|row| row.crate_name == crate_name && row.phase == phase)
            .collect()
    })
}
/// # Errors
/// Returns a parse error when `source` is malformed.
pub fn rows_for_crate_from_str(
    source: &str,
    crate_name: &str,
    phase: u8,
) -> Result<Vec<Row>, OwnershipError> {
    rows_from_str(source).map(|rows| {
        rows.into_iter()
            .filter(|row| row.crate_name == crate_name && row.phase == phase)
            .collect()
    })
}

/// # Errors
/// Returns a parse or missing-row error when the crate has no Phase 1 rows.
pub fn assert_crate_coverage(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    crate_name: &str,
) -> Result<(), OwnershipError> {
    let rows = rows_for_crate(crate_name, 1)?;
    if rows.is_empty() {
        return Err(OwnershipError::NoRows(crate_name.into()));
    }
    check_rows(runtime, ctx, &rows)
}
/// # Errors
/// Returns a parse or missing-row error when the crate has no Phase 1 rows.
pub fn assert_crate_coverage_from_table(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    source: &str,
    crate_name: &str,
) -> Result<(), OwnershipError> {
    let rows = rows_for_crate_from_str(source, crate_name, 1)?;
    if rows.is_empty() {
        return Err(OwnershipError::NoRows(crate_name.into()));
    }
    check_rows(runtime, ctx, &rows)
}

/// Strictly verify the function bindings owned by a crate.
///
/// In addition to the runtime function registry checked by
/// [`assert_crate_coverage`], this opt-in check verifies the symbol's function
/// cell. This catches registrations that are visible to the registry but not
/// callable through the symbol.
///
/// # Errors
/// Returns a parse, missing-row, or binding error.
pub fn assert_crate_function_bindings(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    crate_name: &str,
) -> Result<(), OwnershipError> {
    let rows = rows_for_crate(crate_name, 1)?;
    if rows.is_empty() {
        return Err(OwnershipError::NoRows(crate_name.into()));
    }
    check_function_bindings(runtime, ctx, &rows)
}

/// Strictly verify function bindings using an explicit ownership table.
///
/// # Errors
/// Returns a parse, missing-row, or binding error.
pub fn assert_crate_function_bindings_from_table(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    source: &str,
    crate_name: &str,
) -> Result<(), OwnershipError> {
    let rows = rows_for_crate_from_str(source, crate_name, 1)?;
    if rows.is_empty() {
        return Err(OwnershipError::NoRows(crate_name.into()));
    }
    check_function_bindings(runtime, ctx, &rows)
}

fn check_rows(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    rows: &[Row],
) -> Result<(), OwnershipError> {
    let mut missing = Vec::new();
    for row in rows {
        let Some(package) = runtime.find_package(ctx, &row.package) else {
            for kind in &row.kind {
                missing.push(Missing {
                    package: row.package.clone(),
                    symbol: row.symbol.clone(),
                    kind: *kind,
                    reason: "package not found",
                });
            }
            continue;
        };
        let name = make_string(ctx, runtime, &row.symbol.chars().collect::<Vec<_>>())?;
        let Some((symbol, _)) = Package::from(package).find_symbol(ctx, name)? else {
            for kind in &row.kind {
                missing.push(Missing {
                    package: row.package.clone(),
                    symbol: row.symbol.clone(),
                    kind: *kind,
                    reason: "symbol not interned",
                });
            }
            continue;
        };
        for kind in &row.kind {
            let ok = match kind {
                Kind::Function => runtime
                    .function(ctx, &row.package, &row.symbol)
                    .is_some_and(|value| value != Word::UNBOUND),
                Kind::Class | Kind::Condition => runtime.class(ctx, &row.symbol).is_some(),
                Kind::Macro => symbol_flags(ctx, symbol)? & 4 != 0,
                Kind::Variable => symbol_flags(ctx, symbol)? & 1 != 0,
                Kind::Constant => symbol_flags(ctx, symbol)? & 2 != 0,
                Kind::Other | Kind::SpecialOperator | Kind::Type => true,
            };
            if !ok {
                let reason = match kind {
                    Kind::Function => "function unbound",
                    Kind::Class | Kind::Condition => "class not registered",
                    Kind::Macro => "macro bit not set",
                    Kind::Variable => "special bit not set",
                    Kind::Constant => "constant bit not set",
                    _ => "not registered",
                };
                missing.push(Missing {
                    package: row.package.clone(),
                    symbol: row.symbol.clone(),
                    kind: *kind,
                    reason,
                });
            }
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(OwnershipError::Missing(missing))
    }
}

fn check_function_bindings(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    rows: &[Row],
) -> Result<(), OwnershipError> {
    let mut missing = Vec::new();
    for row in rows.iter().filter(|row| row.kind.contains(&Kind::Function)) {
        let Some(package) = runtime.find_package(ctx, &row.package) else {
            missing.push(Missing {
                package: row.package.clone(),
                symbol: row.symbol.clone(),
                kind: Kind::Function,
                reason: "package not found",
            });
            continue;
        };
        let name = make_string(ctx, runtime, &row.symbol.chars().collect::<Vec<_>>())?;
        let Some((symbol, _)) = Package::from(package).find_symbol(ctx, name)? else {
            missing.push(Missing {
                package: row.package.clone(),
                symbol: row.symbol.clone(),
                kind: Kind::Function,
                reason: "symbol not interned",
            });
            continue;
        };
        let registered = runtime
            .function(ctx, &row.package, &row.symbol)
            .is_some_and(|value| value != Word::UNBOUND);
        let bound = symbol_function(ctx, symbol)? != Word::UNBOUND;
        if !registered {
            missing.push(Missing {
                package: row.package.clone(),
                symbol: row.symbol.clone(),
                kind: Kind::Function,
                reason: "function unbound",
            });
        } else if !bound {
            missing.push(Missing {
                package: row.package.clone(),
                symbol: row.symbol.clone(),
                kind: Kind::Function,
                reason: "symbol function cell unbound",
            });
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(OwnershipError::Missing(missing))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FUNCTION_TABLE: &str = "package\tsymbol\tkind\tcrate\tphase\tdirect-expansion\tnotes\nTEST\tFOO\tfunction\ttest\t1\tno\t\n";

    #[test]
    fn strict_function_bindings_check_the_symbol_function_cell() -> Result<(), OwnershipError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        let package = runtime.ensure_package(&mut ctx, "TEST")?;
        Package::from(package)
            .intern(&mut ctx, &runtime, "FOO")
            .map(|_| ())?;
        runtime.define_function(&mut ctx, "TEST", "FOO", Word::NIL)?;

        assert_crate_coverage_from_table(&runtime, &mut ctx, FUNCTION_TABLE, "test")?;
        let error = match assert_crate_function_bindings_from_table(
            &runtime,
            &mut ctx,
            FUNCTION_TABLE,
            "test",
        ) {
            Ok(()) => {
                return Err(OwnershipError::NoRows(
                    "strict check unexpectedly passed".into(),
                ))
            }
            Err(error) => error,
        };
        assert!(
            matches!(error, OwnershipError::Missing(items) if items[0].reason == "symbol function cell unbound")
        );
        Ok(())
    }
}
