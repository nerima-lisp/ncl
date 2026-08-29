#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_package_introspection_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(name, "FIND-PACKAGE" | "PACKAGE-NAME" | "PACKAGE-USE-LIST") {
            return None;
        }
        Some((|| -> Result<Value, RuntimeError> {
            match name {
                "FIND-PACKAGE" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("find-package", "one", arguments.len()));
                    }
                    let package = Self::package_designator_name(&arguments[0], span)?;
                    let packages = self.packages.borrow();
                    Ok(if packages.package_exists(&package) {
                        Value::package(packages.canonical_package_name(&package))
                    } else {
                        Value::Nil
                    })
                }
                "PACKAGE-NAME" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("package-name", "one", arguments.len()));
                    }
                    match &arguments[0] {
                        Value::Package(package) => Ok(Value::string(package.as_ref())),
                        other => Err(RuntimeError::Type {
                            expected: "PACKAGE".into(),
                            actual: other.type_name().into(),
                            span: Some(span),
                        }),
                    }
                }
                "PACKAGE-USE-LIST" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("package-use-list", "one", arguments.len()));
                    }
                    match &arguments[0] {
                        Value::Package(package) => Ok(Value::list(
                            self.packages
                                .borrow()
                                .use_packages_for(package)
                                .into_iter()
                                .map(Value::package)
                                .collect(),
                        )),
                        other => Err(RuntimeError::Type {
                            expected: "PACKAGE".into(),
                            actual: other.type_name().into(),
                            span: Some(span),
                        }),
                    }
                }
                _ => unreachable!("package introspection primitive name was prevalidated"),
            }
        })())
    }
}
