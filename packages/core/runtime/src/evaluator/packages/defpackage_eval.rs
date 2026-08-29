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
