use super::{normalize_name, unqualified_name, Environment, Form, FormKind, Runtime, RuntimeError};
use crate::value::MethodSpecializer;

pub(super) struct DefmethodParameters {
    pub(super) required: Vec<String>,
    pub(super) required_escaped: Vec<bool>,
    pub(super) specializers: Vec<MethodSpecializer>,
    pub(super) normalized: Vec<Form>,
    pub(super) required_count: usize,
}

impl Runtime {
    pub(super) fn parse_defmethod_required_parameters(
        &self,
        parameters: &[Form],
        environment: &Environment,
    ) -> Result<DefmethodParameters, RuntimeError> {
        let mut required = Vec::new();
        let mut required_escaped = Vec::new();
        let mut specializers = Vec::new();
        let mut normalized_parameters = Vec::new();
        let mut required_parameter_count = 0;
        for parameter in parameters {
            if matches!(&parameter.kind, FormKind::Atom(name) if normalize_name(name).starts_with('&'))
            {
                break;
            }
            let (name_form, specializer_form) = match &parameter.kind {
                FormKind::Atom(_) => (parameter, None),
                FormKind::List(parts) if (1..=2).contains(&parts.len()) => {
                    (&parts[0], parts.get(1))
                }
                _ => {
                    return Err(Self::invalid(
                        "defmethod parameter must be a variable or (variable class)",
                        parameter.span,
                    ));
                }
            };
            let (parameter_name, escaped) =
                Self::variable_name_info(name_form, "defmethod parameter must be a variable")?;
            required.push(unqualified_name(&parameter_name));
            required_escaped.push(escaped);
            let specializer = match specializer_form {
                None => MethodSpecializer::Class("T".into()),
                Some(form) => match &form.kind {
                    FormKind::List(parts) if parts.len() == 2 => {
                        let name =
                            Self::definition_name_from_form(&parts[0], "defmethod specializer")?;
                        if name != "EQL" {
                            return Err(Self::invalid(
                                "unsupported defmethod specializer",
                                parameter.span,
                            ));
                        }
                        MethodSpecializer::Eql(self.eval_in(&parts[1], environment)?)
                    }
                    _ => {
                        let name = Self::definition_name_from_form(form, "defmethod specializer")?;
                        if !crate::builtins::known_type_name(&name, environment) {
                            return Err(Self::invalid(
                                "unknown defmethod specializer",
                                parameter.span,
                            ));
                        }
                        MethodSpecializer::Class(name.into())
                    }
                },
            };
            specializers.push(specializer);
            normalized_parameters.push(name_form.clone());
            required_parameter_count += 1;
        }
        Ok(DefmethodParameters {
            required,
            required_escaped,
            specializers,
            normalized: normalized_parameters,
            required_count: required_parameter_count,
        })
    }
}
