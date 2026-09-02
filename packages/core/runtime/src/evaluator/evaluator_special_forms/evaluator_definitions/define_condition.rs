#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn special_define_condition(
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(Self::arity("define-condition", "four", items.len().saturating_sub(1)));
        }
        let name = unqualified_name(&Self::variable_name(
            &items[1],
            "define-condition name must be a symbol",
        )?);
        let parents = Self::list_form_items(&items[2], "define-condition parent list")?;
        let mut parent_names = Vec::new();
        for parent in parents {
            let parent = Self::definition_name_from_form(parent, "define-condition parent")?;
            if !matches!(parent.as_str(), "CONDITION" | "SERIOUS-CONDITION" | "WARNING" | "ERROR")
                && environment.lookup_condition(&parent).is_none()
            {
                return Err(Self::invalid("unknown define-condition parent", items[2].span));
            }
            parent_names.push(parent);
        }
        let slots = Self::list_form_items(&items[3], "define-condition slot list")?;
        let mut initargs = Vec::new();
        let mut initforms = Vec::new();
        for slot in slots {
            let (slot_name, options) = match &slot.kind {
                FormKind::Atom(_) => (slot, &[][..]),
                FormKind::List(values) if !values.is_empty() => (&values[0], &values[1..]),
                _ => return Err(Self::invalid("define-condition slot must be a symbol or non-empty list", slot.span)),
            };
            let slot_name = unqualified_name(&Self::variable_name(slot_name, "define-condition slot must be a symbol")?);
            if !options.len().is_multiple_of(2) {
                return Err(Self::invalid("define-condition slot options require a value", slot.span));
            }
            for pair in options.as_chunks::<2>().0 {
                let option = Self::definition_name_from_form(&pair[0], "define-condition slot option")?;
                match option.as_str() {
                    "READER" | "WRITER" | "ACCESSOR" => {
                        let accessor = unqualified_name(&Self::variable_name(&pair[1], "condition accessor must be a symbol")?);
                        if option == "READER" || option == "ACCESSOR" {
                            environment.define_function(&accessor, Value::condition_reader(name.clone(), slot_name.clone()));
                        }
                        if option == "WRITER" {
                            environment.define_function(&accessor, Value::condition_writer(name.clone(), slot_name.clone()));
                        }
                    }
                    "INITARG" => {
                        let initarg = Self::definition_name_from_form(&pair[1], "condition initarg")?;
                        initargs.push((initarg, slot_name.clone()));
                    }
                    "INITFORM" => initforms.push((slot_name.clone(), pair[1].clone())),
                    "TYPE" | "DOCUMENTATION" => {}
                    _ => return Err(Self::invalid("unsupported define-condition slot option", pair[0].span)),
                }
            }
        }
        for option in items.iter().skip(4) {
            let values = Self::list_form_items(option, "define-condition option")?;
            if values.is_empty() {
                return Err(Self::invalid("define-condition option must be non-empty", option.span));
            }
            let option_name = Self::definition_name_from_form(&values[0], "define-condition option name")?;
            if option_name == "DOCUMENTATION" && (values.len() != 2 || !matches!(values[1].kind, FormKind::String(_))) {
                return Err(Self::invalid("define-condition :documentation needs one string", option.span));
            }
        }
        environment.define_condition(name.clone(), crate::environment::ConditionDefinition {
            parents: parent_names,
            initargs,
            initforms,
        });
        Ok(Value::symbol(name))
    }
}
