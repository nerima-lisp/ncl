use super::{
    Environment, Form, FormKind, HashSet, Runtime, RuntimeError, StructureDefinition,
    StructureSlot, unqualified_name,
};

impl Runtime {
    pub(super) fn collect_defstruct_slots(
        &self,
        structure_name: &str,
        included_structure: Option<(StructureDefinition, Vec<Form>)>,
        slot_forms: &[Form],
        environment: &Environment,
    ) -> Result<(Vec<String>, Vec<StructureSlot>), RuntimeError> {
        let mut structure_types = vec![structure_name.to_owned()];
        let mut slots = Vec::new();
        let mut slot_names = HashSet::new();
        if let Some((parent, overrides)) = included_structure {
            structure_types.extend(parent.type_names.clone());
            slots = parent.slots;
            for slot in &slots {
                slot_names.insert(slot.name.clone());
            }
            let mut overridden_slots = HashSet::new();
            for slot_form in overrides {
                let (raw_slot_name, init_form, read_only) =
                    self.defstruct_slot_description(&slot_form, environment)?;
                let slot_name = unqualified_name(&raw_slot_name);
                let Some(slot) = slots.iter_mut().find(|slot| slot.name == slot_name) else {
                    return Err(Self::invalid(
                        "defstruct :include slot must name an inherited slot",
                        slot_form.span,
                    ));
                };
                if !overridden_slots.insert(slot_name) {
                    return Err(Self::invalid(
                        "defstruct :include cannot override a slot more than once",
                        slot_form.span,
                    ));
                }
                if let Some(init_form) = init_form {
                    slot.init_form = Some(init_form);
                }
                if let Some(read_only) = read_only {
                    slot.read_only = read_only;
                }
            }
        }
        for slot_form in slot_forms {
            let (raw_slot_name, init_form, read_only) =
                self.defstruct_slot_description(slot_form, environment)?;
            let slot_name = unqualified_name(&raw_slot_name);
            if !slot_names.insert(slot_name.clone()) {
                return Err(Self::invalid(
                    "defstruct cannot define duplicate slots",
                    slot_form.span,
                ));
            }
            slots.push(StructureSlot {
                name: slot_name,
                init_form,
                read_only: read_only.unwrap_or(false),
            });
        }
        Ok((structure_types, slots))
    }

    fn defstruct_slot_description(
        &self,
        slot_form: &Form,
        environment: &Environment,
    ) -> Result<(String, Option<Form>, Option<bool>), RuntimeError> {
        match &slot_form.kind {
            FormKind::Atom(_) => Ok((
                Self::variable_name_info(
                    slot_form,
                    "defstruct slot must be a symbol or a slot specification",
                )?
                .0,
                None,
                None,
            )),
            FormKind::List(slot_items) if (1..=3).contains(&slot_items.len()) => {
                let slot_name = Self::variable_name_info(
                    &slot_items[0],
                    "defstruct slot name must be a symbol",
                )?;
                let read_only = slot_items
                    .get(2)
                    .map(|form| {
                        self.eval_in(form, environment)
                            .map(|value| value.is_truthy())
                    })
                    .transpose()?;
                Ok((slot_name.0, slot_items.get(1).cloned(), read_only))
            }
            _ => Err(Self::invalid(
                "defstruct slot must be a symbol or a one- to three-element list",
                slot_form.span,
            )),
        }
    }
}
