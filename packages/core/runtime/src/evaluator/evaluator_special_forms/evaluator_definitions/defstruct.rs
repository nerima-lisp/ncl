#![allow(clippy::wildcard_imports)]
use super::*;

mod option_values;
mod options;
mod registration;
mod slots;

use options::DefstructOptions;
use registration::DefstructRegistration;

impl Runtime {
    pub(crate) fn special_defstruct(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                "defstruct",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let (name_form, option_forms, slot_forms) = match &items[1].kind {
            FormKind::Atom(_) => (&items[1], &items[2..2], &items[2..]),
            FormKind::List(name_and_options) if !name_and_options.is_empty() => {
                (&name_and_options[0], &name_and_options[1..], &items[2..])
            }
            _ => {
                return Err(Self::invalid(
                    "defstruct name must be a symbol or a name-and-options list",
                    items[1].span,
                ));
            }
        };
        let (raw_name, _) = Self::variable_name_info(name_form, "defstruct name must be a symbol")?;
        let structure_name = unqualified_name(&raw_name);
        let options = Self::parse_defstruct_options(&structure_name, option_forms, environment)?;
        let DefstructOptions {
            conc_name,
            predicate_name,
            copier_name,
            constructor_options,
            included_structure,
        } = options;
        let (structure_types, slots) = self.collect_defstruct_slots(
            &structure_name,
            included_structure,
            slot_forms,
            environment,
        )?;

        Self::register_defstruct(
            environment,
            DefstructRegistration {
                structure_name: structure_name.clone(),
                structure_types,
                slots,
                constructor_options,
                predicate_name,
                copier_name,
                conc_name,
            },
        );
        Ok(Value::symbol(structure_name))
    }
}
