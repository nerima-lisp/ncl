use ncl_syntax::{Form, FormKind};

use crate::environment::normalize_name;
use crate::evaluator::helpers::atom_name;
use crate::{Runtime, RuntimeError};

use super::defpackage_types::{DefpackageBuilder, DefpackageSpec};

impl Runtime {
    pub(super) fn parse_defpackage(items: &[Form]) -> Result<DefpackageSpec, RuntimeError> {
        let name = Self::package_name_from_form(&items[0])?;
        let mut builder = DefpackageBuilder::new();

        for option in items.iter().skip(1) {
            let FormKind::List(option_items) = &option.kind else {
                return Err(Self::invalid(
                    "defpackage option must be a list",
                    option.span,
                ));
            };
            let Some(option_name) = option_items.first().and_then(atom_name) else {
                return Err(Self::invalid("defpackage option needs a name", option.span));
            };
            let normalized_option = normalize_name(option_name);
            let trimmed = normalized_option.trim_start_matches(':');
            if Self::apply_defpackage_metadata_option(
                &mut builder,
                trimmed,
                option_items,
                option.span,
            )? {
                continue;
            }
            if Self::apply_defpackage_symbol_option(
                &mut builder,
                trimmed,
                option_items,
                option.span,
            )? {
                continue;
            }
            return Err(Self::invalid(
                "unsupported defpackage option",
                option_items[0].span,
            ));
        }
        Ok(builder.into_spec(name))
    }
}

#[cfg(test)]
mod tests {
    use ncl_syntax::{Form, FormKind, Span};

    use crate::Runtime;

    use super::super::defpackage_types::DefpackageOperation;

    const SPAN: Span = Span::new(0, 1);

    fn atom(name: &str) -> Form {
        Form::atom(name, SPAN)
    }

    fn string(value: &str) -> Form {
        Form::new(FormKind::String(value.to_string()), SPAN)
    }

    fn valid<T, E>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|_| panic!("expected a valid defpackage spec"))
    }

    #[test]
    fn defpackage_parser_accepts_all_options() {
        let options = vec![
            atom("TOOLS"),
            Form::list(vec![atom(":nicknames"), string("T")], SPAN),
            Form::list(vec![atom(":use"), atom("COMMON-LISP")], SPAN),
            Form::list(vec![atom(":documentation"), string("tool package")], SPAN),
            Form::list(vec![atom(":size"), atom("16")], SPAN),
            Form::list(
                vec![
                    atom(":local-nicknames"),
                    Form::list(vec![atom("CL"), atom("COMMON-LISP")], SPAN),
                ],
                SPAN,
            ),
            Form::list(vec![atom(":export"), atom("run")], SPAN),
            Form::list(vec![atom(":shadow"), atom("print")], SPAN),
            Form::list(vec![atom(":intern"), atom("state")], SPAN),
            Form::list(
                vec![atom(":import-from"), atom("COMMON-LISP"), atom("car")],
                SPAN,
            ),
            Form::list(
                vec![
                    atom(":shadowing-import-from"),
                    atom("COMMON-LISP"),
                    atom("cdr"),
                ],
                SPAN,
            ),
        ];
        let spec = valid(Runtime::parse_defpackage(&options));

        assert_eq!(spec.name, "TOOLS");
        assert_eq!(spec.nicknames, ["T"]);
        assert_eq!(spec.use_packages, ["COMMON-LISP"]);
        assert_eq!(spec.documentation.as_deref(), Some("tool package"));
        assert!(spec.exports.contains("RUN"));
        assert_eq!(
            spec.local_nicknames.get("COMMON-LISP"),
            Some(&"COMMON-LISP".to_string())
        );
        assert!(
            matches!(spec.operations[0], DefpackageOperation::Shadow(ref name) if name == "PRINT")
        );
        assert!(
            matches!(spec.operations[1], DefpackageOperation::Intern(ref name) if name == "STATE")
        );
        assert!(matches!(
            spec.operations[2],
            DefpackageOperation::Import {
                shadowing: false,
                ..
            }
        ));
        assert!(matches!(
            spec.operations[3],
            DefpackageOperation::Import {
                shadowing: true,
                ..
            }
        ));
    }

    #[test]
    fn defpackage_parser_rejects_malformed_options() {
        let cases = [
            vec![atom("TOOLS"), atom("not-a-list")],
            vec![atom("TOOLS"), Form::list(vec![], SPAN)],
            vec![
                atom("TOOLS"),
                Form::list(vec![atom(":size"), atom("-1")], SPAN),
            ],
            vec![
                atom("TOOLS"),
                Form::list(vec![atom(":local-nicknames"), atom("CL")], SPAN),
            ],
            vec![atom("TOOLS"), Form::list(vec![atom(":import-from")], SPAN)],
            vec![atom("TOOLS"), Form::list(vec![atom(":unknown")], SPAN)],
            vec![Form::new(FormKind::String(String::new()), SPAN)],
        ];

        for items in cases {
            assert!(Runtime::parse_defpackage(&items).is_err());
        }
    }
}
