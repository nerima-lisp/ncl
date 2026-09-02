use std::rc::Rc;

use super::{ClassSlot, Environment, Form, FormKind, Runtime, RuntimeError, Span};

impl Runtime {
    pub(super) fn parse_defclass_option(
        option: &Form,
        default_initargs: &mut Vec<(String, Form)>,
        documentation: &mut Option<String>,
    ) -> Result<(), RuntimeError> {
        let option_items = Self::list_form_items(option, "defclass option")?;
        if option_items.is_empty() {
            return Err(Self::invalid(
                "defclass option must be a non-empty list",
                option.span,
            ));
        }
        let option_name =
            Self::definition_name_from_form(&option_items[0], "defclass option name")?;
        match option_name.as_str() {
            "DEFAULT-INITARGS" => {
                if option_items.len() < 3 || !(option_items.len() - 1).is_multiple_of(2) {
                    return Err(Self::invalid(
                        "defclass :default-initargs requires initarg and form pairs",
                        option.span,
                    ));
                }
                for pair in option_items[1..].as_chunks::<2>().0 {
                    let initarg =
                        Self::definition_name_from_form(&pair[0], "defclass default initarg")?;
                    if let Some(existing) = default_initargs
                        .iter_mut()
                        .find(|(name, _)| name == &initarg)
                    {
                        existing.1 = pair[1].clone();
                    } else {
                        default_initargs.push((initarg, pair[1].clone()));
                    }
                }
            }
            "DOCUMENTATION"
                if option_items.len() != 2
                    || !matches!(option_items[1].kind, FormKind::String(_)) =>
            {
                return Err(Self::invalid(
                    "defclass :documentation needs one string",
                    option.span,
                ));
            }
            "DOCUMENTATION" => {
                if let FormKind::String(value) = &option_items[1].kind {
                    *documentation = Some(value.to_string());
                }
            }
            _ => {
                return Err(Self::invalid("unsupported defclass option", option.span));
            }
        }
        Ok(())
    }

    pub(super) fn merge_defclass_superclasses(
        class_name: &str,
        direct_superclasses: &[String],
        slots: &mut Vec<ClassSlot>,
        default_initargs: &mut Vec<(String, Form)>,
        environment: &Environment,
        span: Span,
    ) -> Result<Vec<Rc<str>>, RuntimeError> {
        let mut resolved_superclasses = Vec::new();
        for superclass in direct_superclasses {
            if superclass == "OBJECT" || superclass == "STANDARD-OBJECT" {
                if !resolved_superclasses.iter().any(|name: &Rc<str>| name.as_ref() == "STANDARD-OBJECT") {
                    resolved_superclasses.push("STANDARD-OBJECT".to_owned().into());
                }
                continue;
            }
            let Some(definition) = environment.lookup_class(superclass) else {
                return Err(Self::invalid("unknown defclass superclass", span));
            };
            resolved_superclasses.push(definition.name.clone().into());
            for inherited in &definition.slots {
                if !slots.iter().any(|slot| slot.name == inherited.name) {
                    slots.push(inherited.clone());
                }
            }
            for inherited in &definition.default_initargs {
                if !default_initargs
                    .iter()
                    .any(|(name, _)| name == &inherited.0)
                {
                    default_initargs.push(inherited.clone());
                }
            }
        }
        if resolved_superclasses.is_empty() {
            resolved_superclasses.push("STANDARD-OBJECT".to_owned().into());
        }
        let mut sequences = resolved_superclasses
            .iter()
            .map(|name| environment.lookup_class(name).map(|class| class.precedence.clone()).unwrap_or_else(|| vec![name.clone()]))
            .collect::<Vec<_>>();
        sequences.push(resolved_superclasses.clone());
        let mut precedence = vec![class_name.to_owned().into()];
        while sequences.iter().any(|sequence| !sequence.is_empty()) {
            let candidate = sequences.iter().filter_map(|sequence| sequence.first()).find(|candidate| {
                sequences.iter().all(|sequence| !sequence.iter().skip(1).any(|name| name == *candidate))
            }).cloned().ok_or_else(|| Self::invalid("inconsistent class precedence order", span))?;
            precedence.push(candidate.clone());
            for sequence in &mut sequences {
                if sequence.first() == Some(&candidate) {
                    sequence.remove(0);
                }
            }
        }
        Ok(precedence)
    }
}
