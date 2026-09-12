#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_package_lifecycle_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(name, "RENAME-PACKAGE" | "DELETE-PACKAGE") {
            return None;
        }
        Some((|| -> Result<Value, RuntimeError> {
            match name {
                "RENAME-PACKAGE" => {
                    if !(2..=3).contains(&arguments.len()) {
                        return Err(Self::arity(
                            "rename-package",
                            "two or three",
                            arguments.len(),
                        ));
                    }
                    let old_name = self.package_name_from_value(&arguments[0], span)?;
                    let new_name = Self::package_designator_name(&arguments[1], span)?;
                    let nicknames = match arguments.get(2) {
                        Some(value) => self.package_nickname_names_from_value(value, span)?,
                        None => Vec::new(),
                    };
                    let renamed_name = {
                        let mut packages = self.packages.borrow_mut();
                        packages
                            .rename_package(&old_name, &new_name, nicknames)
                            .map_err(|message| Self::package_error(&message, span))?
                    };
                    self.rename_package_prefix(&old_name, &renamed_name);
                    self.packages
                        .borrow()
                        .package_object_for(&renamed_name)
                        .map(Value::package_object)
                        .ok_or_else(|| Self::package_error("unknown package", span))
                }
                "DELETE-PACKAGE" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("delete-package", "one", arguments.len()));
                    }
                    let package = self.package_name_from_value(&arguments[0], span)?;
                    self.packages
                        .borrow_mut()
                        .delete_package(&package)
                        .map_err(|message| Self::package_error(&message, span))?;
                    Ok(Value::boolean(true))
                }
                _ => unreachable!("package lifecycle primitive name was prevalidated"),
            }
        })())
    }
}
