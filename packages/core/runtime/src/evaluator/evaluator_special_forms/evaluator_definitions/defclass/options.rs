use super::{ClassSlot, Environment, Form, Runtime, RuntimeError, Span};

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
            "METACLASS" => {
                let metaclass = Self::definition_name_from_form(
                    option_items.get(1).ok_or_else(|| {
                        Self::invalid("unsupported defclass metaclass", option.span)
                    })?,
                    "defclass metaclass",
                )?;
                if option_items.len() != 2 || metaclass != "STANDARD-CLASS" {
                    return Err(Self::invalid("unsupported defclass metaclass", option.span));
                }
            }
            "DEFAULT-INITARGS" => {
                if option_items.len() == 1 {
                    return Ok(());
                }
                if !(option_items.len() - 1).is_multiple_of(2) {
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
                if option_items.len() != 2 || Self::form_string(&option_items[1]).is_none() =>
            {
                return Err(Self::invalid(
                    "defclass :documentation needs one string",
                    option.span,
                ));
            }
            "DOCUMENTATION" => {
                *documentation = Self::form_string(&option_items[1]).map(str::to_owned);
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
    ) -> Result<Vec<String>, RuntimeError> {
        let mut precedence = vec![class_name.to_owned()];
        let mut sequences = Vec::new();
        for superclass in direct_superclasses {
            if superclass == "OBJECT" || superclass == "STANDARD-OBJECT" {
                sequences.push(vec!["STANDARD-OBJECT".to_owned()]);
                continue;
            }
            let Some(definition) = environment.lookup_class(superclass) else {
                return Err(Self::invalid("unknown defclass superclass", span));
            };
            sequences.push(
                definition
                    .precedence
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            );
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
        sequences.push(direct_superclasses.iter().map(ToOwned::to_owned).collect());
        while sequences.iter().any(|sequence| !sequence.is_empty()) {
            let candidate = sequences
                .iter()
                .filter_map(|sequence| sequence.first())
                .find(|head| {
                    !sequences
                        .iter()
                        .any(|sequence| sequence.iter().skip(1).any(|tail| tail == *head))
                })
                .cloned()
                .ok_or_else(|| Self::invalid("inconsistent class precedence list", span))?;
            precedence.push(candidate.clone());
            for sequence in &mut sequences {
                if sequence.first() == Some(&candidate) {
                    sequence.remove(0);
                }
            }
        }
        if !precedence.iter().any(|name| name == "STANDARD-OBJECT") {
            precedence.push("STANDARD-OBJECT".to_owned());
        }
        Ok(precedence)
    }
}
