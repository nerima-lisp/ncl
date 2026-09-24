//! Parsing and querying of the embedded symbol-ownership table.
//!
//! The table is `conformance/ownership/symbols.tsv`, embedded with
//! [`include_str!`] so the gate never depends on the working directory.

use crate::coverage::OwnershipError;

/// The embedded ownership table, compiled into this crate.
const TABLE: &str = include_str!("../../../conformance/ownership/symbols.tsv");

/// Number of tab-separated columns in a table row.
const COLUMNS: usize = 7;

/// Symbol kinds recorded in the ownership table.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Kind {
    /// A CLOS class.
    Class,
    /// A condition type.
    Condition,
    /// A constant variable.
    Constant,
    /// A function.
    Function,
    /// A macro.
    Macro,
    /// A kind with no dedicated registry in the gate.
    Other,
    /// A special operator.
    SpecialOperator,
    /// A type specifier name.
    Type,
    /// A variable.
    Variable,
}

impl Kind {
    /// Parse one kind token from the `kind` column.
    fn parse(token: &str) -> Option<Self> {
        Some(match token {
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

/// One parsed row of the ownership table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Row {
    /// Owning package name, for example `COMMON-LISP`.
    pub package: String,
    /// Symbol name, for example `CAR`.
    pub symbol: String,
    /// One or more kinds recorded for the symbol.
    pub kind: Vec<Kind>,
    /// Crate that owns the symbol, for example `ncl-types`.
    pub crate_name: String,
    /// Implementation phase in which the crate registers the symbol.
    pub phase: u8,
    /// Whether the symbol is visible through a direct expansion.
    pub direct_expansion: bool,
}

/// Parse every data row of the embedded ownership table.
///
/// The first line is a header and is skipped.
///
/// # Errors
///
/// Returns [`OwnershipError::BadRow`] when the embedded table is malformed.
pub fn rows() -> Result<Vec<Row>, OwnershipError> {
    TABLE
        .lines()
        .enumerate()
        .skip(1)
        .map(|(index, line)| parse_row(index + 1, line))
        .collect()
}

/// Return the ownership rows for one crate and phase.
///
/// # Errors
///
/// Returns [`OwnershipError::BadRow`] when the embedded table is malformed.
pub fn rows_for_crate(crate_name: &str, phase: u8) -> Result<Vec<Row>, OwnershipError> {
    Ok(rows()?
        .into_iter()
        .filter(|row| row.crate_name == crate_name && row.phase == phase)
        .collect())
}

/// Parse one non-header table line.
fn parse_row(line_number: usize, line: &str) -> Result<Row, OwnershipError> {
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() != COLUMNS {
        return Err(OwnershipError::bad_row(
            line_number,
            "expected seven tab-separated columns",
        ));
    }
    let phase = fields[4]
        .parse::<u8>()
        .map_err(|_| OwnershipError::bad_row(line_number, "invalid phase"))?;
    let direct_expansion = match fields[5] {
        "yes" => true,
        "no" => false,
        _ => {
            return Err(OwnershipError::bad_row(
                line_number,
                "invalid direct-expansion flag",
            ));
        }
    };
    let mut kind = Vec::new();
    for token in fields[2].split('+') {
        match Kind::parse(token) {
            Some(parsed) => kind.push(parsed),
            None => return Err(OwnershipError::bad_row(line_number, "invalid kind")),
        }
    }
    Ok(Row {
        package: fields[0].to_owned(),
        symbol: fields[1].to_owned(),
        kind,
        crate_name: fields[3].to_owned(),
        phase,
        direct_expansion,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests assert on parser failures")]
mod tests {
    use super::{Kind, OwnershipError, parse_row, rows};

    #[test]
    fn embedded_table_parses_completely() {
        let source = include_str!("../../../conformance/ownership/symbols.tsv");
        let parsed = rows().unwrap();
        assert_eq!(parsed.len(), source.lines().count() - 1);
        assert!(parsed.iter().all(|row| (1..=3).contains(&row.phase)));
    }

    #[test]
    fn compound_kind_expands_to_every_kind() {
        let row = parse_row(
            2,
            "COMMON-LISP\tFOO\tclass+function\tncl-types\t1\tno\tnote",
        )
        .unwrap();
        assert_eq!(row.kind, vec![Kind::Class, Kind::Function]);
    }

    #[test]
    fn short_row_is_rejected() {
        let error = parse_row(2, "COMMON-LISP\tFOO\tclass\tncl-types\t1\tno").unwrap_err();
        assert!(error.to_string().contains("symbols.tsv line 2"));
        assert!(matches!(error, OwnershipError::BadRow { line: 2, .. }));
    }

    #[test]
    fn every_kind_has_a_display_name() {
        let kinds = [
            Kind::Class,
            Kind::Condition,
            Kind::Constant,
            Kind::Function,
            Kind::Macro,
            Kind::Other,
            Kind::SpecialOperator,
            Kind::Type,
            Kind::Variable,
        ];
        for kind in kinds {
            assert!(!kind.to_string().is_empty());
        }
    }

    #[test]
    fn invalid_kind_is_rejected() {
        let error =
            parse_row(2, "COMMON-LISP\tFOO\tclass+bogus\tncl-types\t1\tno\tnote").unwrap_err();
        assert!(matches!(error, OwnershipError::BadRow { line: 2, .. }));
    }

    #[test]
    fn invalid_phase_is_rejected() {
        let error = parse_row(2, "COMMON-LISP\tFOO\tclass\tncl-types\tx\tno\tnote").unwrap_err();
        assert!(matches!(error, OwnershipError::BadRow { line: 2, .. }));
    }

    #[test]
    fn invalid_direct_expansion_is_rejected() {
        let error = parse_row(2, "COMMON-LISP\tFOO\tclass\tncl-types\t1\tmaybe\tnote").unwrap_err();
        assert!(matches!(error, OwnershipError::BadRow { line: 2, .. }));
    }
}
