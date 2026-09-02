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
        let mut precedence = vec![class_name.to_owned().into()];
        for superclass in direct_superclasses {
            if superclass == "OBJECT" || superclass == "STANDARD-OBJECT" {
                if !precedence
                    .iter()
                    .any(|name: &Rc<str>| name.as_ref() == "STANDARD-OBJECT")
                {
                    precedence.push("STANDARD-OBJECT".to_owned().into());
                }
                continue;
            }
            let Some(definition) = environment.lookup_class(superclass) else {
                return Err(Self::invalid("unknown defclass superclass", span));
            };
            for name in &definition.precedence {
                if !precedence
                    .iter()
                    .any(|existing| existing.as_ref() == name.as_ref())
                {
                    precedence.push(name.clone());
                }
            }
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
        if !precedence
            .iter()
            .any(|name| name.as_ref() == "STANDARD-OBJECT")
        {
            precedence.push("STANDARD-OBJECT".to_owned().into());
        }
        Ok(precedence)
    }
}
