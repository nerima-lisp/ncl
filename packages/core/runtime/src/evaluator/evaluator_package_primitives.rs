#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_package_listing_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(name, "DOCUMENTATION" | "LIST-ALL-PACKAGES") {
            return None;
        }
        let result = match name {
            "DOCUMENTATION" => match arguments.len() {
                2 => match &arguments[0] {
                    Value::Package(package) => Ok(self
                        .packages
                        .borrow()
                        .package_documentation(package)
                        .map_or(Value::Nil, |documentation| {
                            Value::string(documentation.as_str())
                        })),
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                },
                _ => Err(Self::arity("documentation", "two", arguments.len())),
            },
            "LIST-ALL-PACKAGES" => match arguments {
                [] => {
                    let names = self.packages.borrow().all_package_names();
                    Ok(Value::list(names.into_iter().map(Value::package).collect()))
                }
                _ => Err(Self::arity("list-all-packages", "zero", arguments.len())),
            },
            _ => unreachable!("package listing primitive name was prevalidated"),
        };
        Some(result)
    }

    pub(crate) fn apply_method_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(name, "CALL-NEXT-METHOD" | "NEXT-METHOD-P") {
            return None;
        }
        let result = match name {
            "CALL-NEXT-METHOD" => {
                let (continuation, default_arguments) = {
                    let contexts = self.method_context.borrow();
                    let Some(context) = contexts.last() else {
                        return Some(Err(Self::invalid(
                            "call-next-method is only available in a method",
                            span,
                        )));
                    };
                    (context.next.clone(), context.arguments.clone())
                };
                let Some(continuation) = continuation else {
                    return Some(Err(Self::invalid("no next method is applicable", span)));
                };
                let next_arguments = match arguments {
                    [] => default_arguments,
                    _ => arguments.to_vec(),
                };
                self.invoke_continuation(continuation, &next_arguments, span, environment)
            }
            "NEXT-METHOD-P" => match arguments {
                [] => {
                    let has_next = self
                        .method_context
                        .borrow()
                        .last()
                        .and_then(|context| context.next.as_ref())
                        .is_some();
                    Ok(Value::boolean(has_next))
                }
                _ => Err(Self::arity("next-method-p", "zero", arguments.len())),
            },
            _ => unreachable!("method primitive name was prevalidated"),
        };
        Some(result)
    }
}
