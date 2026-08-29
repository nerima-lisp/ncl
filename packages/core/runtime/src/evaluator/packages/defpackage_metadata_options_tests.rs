#[cfg(test)]
mod tests {
    use ncl_syntax::{Form, FormKind, Span};

    use crate::{Runtime, RuntimeError};

    use super::super::defpackage_types::DefpackageBuilder;

    const SPAN: Span = Span::new(0, 1);

    fn atom(name: &str) -> Form {
        Form::atom(name, SPAN)
    }

    fn string(value: &str) -> Form {
        Form::new(FormKind::String(value.to_string()), SPAN)
    }

    fn apply(option: &str, items: &[Form]) -> Result<bool, RuntimeError> {
        let mut builder = DefpackageBuilder::new();
        Runtime::apply_defpackage_metadata_option(&mut builder, option, items, SPAN)
    }

    #[test]
    fn rejects_a_repeated_nicknames_option() {
        let mut builder = DefpackageBuilder::new();
        let items = [atom(":nicknames"), string("A")];
        assert!(matches!(
            Runtime::apply_defpackage_metadata_option(&mut builder, "NICKNAMES", &items, SPAN),
            Ok(true)
        ));
        assert!(
            Runtime::apply_defpackage_metadata_option(&mut builder, "NICKNAMES", &items, SPAN)
                .is_err()
        );
    }

    #[test]
    fn rejects_a_repeated_use_option() {
        let mut builder = DefpackageBuilder::new();
        let items = [atom(":use"), atom("COMMON-LISP")];
        assert!(matches!(
            Runtime::apply_defpackage_metadata_option(&mut builder, "USE", &items, SPAN),
            Ok(true)
        ));
        assert!(
            Runtime::apply_defpackage_metadata_option(&mut builder, "USE", &items, SPAN).is_err()
        );
    }

    #[test]
    fn rejects_malformed_documentation_options() {
        assert!(apply("DOCUMENTATION", &[atom(":documentation")]).is_err());
        assert!(apply("DOCUMENTATION", &[atom(":documentation"), atom("5")]).is_err());
        assert!(matches!(
            apply("DOCUMENTATION", &[atom(":documentation"), string("hi")]),
            Ok(true)
        ));
    }

    #[test]
    fn rejects_malformed_size_options() {
        assert!(apply("SIZE", &[atom(":size")]).is_err());
        assert!(apply("SIZE", &[atom(":size"), Form::list(vec![atom("1")], SPAN)]).is_err());
        assert!(matches!(
            apply("SIZE", &[atom(":size"), atom("16")]),
            Ok(true)
        ));
    }

    #[test]
    fn rejects_local_nicknames_with_a_malformed_mapping_length() {
        let mapping = Form::list(vec![atom("CL")], SPAN);
        assert!(apply("LOCAL-NICKNAMES", &[atom(":local-nicknames"), mapping]).is_err());
    }

    #[test]
    fn rejects_duplicate_local_nicknames_within_one_option() {
        let items = [
            atom(":local-nicknames"),
            Form::list(vec![atom("CL"), atom("COMMON-LISP")], SPAN),
            Form::list(vec![atom("CL"), atom("KEYWORD")], SPAN),
        ];
        assert!(apply("LOCAL-NICKNAMES", &items).is_err());
    }
}
