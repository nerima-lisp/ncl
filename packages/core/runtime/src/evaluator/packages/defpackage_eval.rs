use ncl_syntax::{Form, Span};

use crate::{Runtime, RuntimeError, Value};

use super::defpackage_types::{DefpackageOperation, DefpackageSpec};

impl Runtime {
    pub(in crate::evaluator) fn special_defpackage(
        &self,
        items: &[Form],
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                "defpackage",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let spec = Self::parse_defpackage(&items[1..])?;
        self.validate_defpackage(&spec, items[1].span)?;
        self.apply_defpackage(spec, items[1].span)
    }

    fn validate_defpackage(&self, spec: &DefpackageSpec, span: Span) -> Result<(), RuntimeError> {
        let packages = self.packages.borrow();
        if let Some(missing) = spec
            .use_packages
            .iter()
            .find(|name| !packages.package_exists(name))
        {
            return Err(Self::package_error(
                &format!("unknown package {missing}"),
                span,
            ));
        }
        for operation in &spec.operations {
            let DefpackageOperation::Import {
                source_package,
                source_name,
                ..
            } = operation
            else {
                continue;
            };
            if !packages.package_exists(source_package) {
                return Err(Self::package_error(
                    &format!("unknown package {source_package}"),
                    span,
                ));
            }
            if !packages.symbol_exists(source_package, source_name) {
                return Err(Self::package_error(
                    &format!("unknown symbol {source_package}::{source_name}"),
                    span,
                ));
            }
        }
        Ok(())
    }

    fn apply_defpackage(&self, spec: DefpackageSpec, span: Span) -> Result<Value, RuntimeError> {
        let mut packages = self.packages.borrow_mut();
        if let Err(message) = packages.define_package(
            &spec.name,
            spec.nicknames,
            spec.use_packages,
            spec.exports,
            spec.documentation,
            spec.local_nicknames,
        ) {
            return Err(Self::package_error(&message, span));
        }
        for operation in spec.operations {
            match operation {
                DefpackageOperation::Shadow(symbol) => packages.shadow_symbol(&spec.name, &symbol),
                DefpackageOperation::Intern(symbol) => {
                    let _ = packages.intern_symbol(&spec.name, &symbol);
                }
                DefpackageOperation::Import {
                    source_package,
                    source_name,
                    shadowing,
                } => packages.import_symbol(&source_package, &source_name, &spec.name, shadowing),
            }
        }
        let canonical_name = packages.canonical_package_name(&spec.name);
        Ok(Value::package(&canonical_name))
    }
}

#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn defpackage_rejects_a_name_only_form() {
        assert!(Runtime::new().eval_source("(defpackage)").is_err());
    }

    #[test]
    fn defpackage_rejects_use_of_an_unknown_package() {
        let runtime = Runtime::new();
        assert!(
            runtime
                .eval_source("(defpackage \"DEFPKG-EVAL-USE\" (:use \"NO-SUCH-PACKAGE\"))")
                .is_err()
        );
    }

    #[test]
    fn defpackage_rejects_import_from_an_unknown_package() {
        let runtime = Runtime::new();
        assert!(
            runtime
                .eval_source(
                    "(defpackage \"DEFPKG-EVAL-IMPORT\" (:import-from \"NO-SUCH-PACKAGE\" \"X\"))"
                )
                .is_err()
        );
    }

    #[test]
    fn defpackage_rejects_import_of_an_unknown_symbol() {
        let runtime = Runtime::new();
        runtime
            .eval_source("(defpackage \"DEFPKG-EVAL-SRC\" (:export \"KNOWN\"))")
            .unwrap_or_else(|error| panic!("expected setup defpackage form to succeed: {error}"));
        assert!(
            runtime
                .eval_source(
                    "(defpackage \"DEFPKG-EVAL-SYMBOL\" (:import-from \"DEFPKG-EVAL-SRC\" \"NO-SUCH-SYMBOL-XYZ\"))"
                )
                .is_err()
        );
    }

    #[test]
    fn defpackage_rejects_a_nickname_already_claimed_by_another_package() {
        let runtime = Runtime::new();
        runtime
            .eval_source("(defpackage \"DEFPKG-EVAL-NICK-A\" (:nicknames \"DEFPKG-EVAL-NICK\"))")
            .unwrap_or_else(|error| panic!("expected setup defpackage form to succeed: {error}"));
        assert!(
            runtime
                .eval_source(
                    "(defpackage \"DEFPKG-EVAL-NICK-B\" (:nicknames \"DEFPKG-EVAL-NICK\"))"
                )
                .is_err()
        );
    }
}
