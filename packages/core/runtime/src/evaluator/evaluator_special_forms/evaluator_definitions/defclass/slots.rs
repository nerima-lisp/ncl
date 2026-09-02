use super::{
    is_nil_form, unqualified_name, ClassSlot, Form, FormKind, Rc, RefCell, Runtime, RuntimeError,
    Value,
};

pub(super) struct DefclassSlotRegistration {
    pub(super) slot: ClassSlot,
    pub(super) readers: Vec<(String, String)>,
    pub(super) writers: Vec<(String, String)>,
}

impl Runtime {
    pub(super) fn parse_defclass_slot(
        slot_form: &Form,
    ) -> Result<DefclassSlotRegistration, RuntimeError> {
        let (slot_name_form, options) = match &slot_form.kind {
            FormKind::Atom(_) => (slot_form, &[][..]),
            FormKind::List(slot_items) if !slot_items.is_empty() => {
                (&slot_items[0], &slot_items[1..])
            }
            _ => {
                return Err(Self::invalid(
                    "defclass slot must be a symbol or non-empty list",
                    slot_form.span,
                ));
            }
        };
        let slot_name = unqualified_name(&Self::variable_name(
            slot_name_form,
            "defclass slot must be a symbol",
        )?);
        let mut initargs = Vec::new();
        let mut documentation = None;
        let mut init_form = None;
        let mut type_form = None;
        let mut class_value = None;
        let mut readers = Vec::new();
        let mut writers = Vec::new();
        if !options.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "defclass slot options require a value",
                slot_form.span,
            ));
        }
        for option in options.as_chunks::<2>().0 {
            let option_name = Self::definition_name_from_form(&option[0], "defclass slot option")?;
            match option_name.as_str() {
                "INITARG" => {
                    if !is_nil_form(&option[1]) {
                        initargs.push(Self::definition_name_from_form(
                            &option[1],
                            "defclass initarg",
                        )?);
                    }
                }
                "INITFORM" => init_form = Some(option[1].clone()),
                "ALLOCATION" => {
                    let allocation =
                        Self::definition_name_from_form(&option[1], "defclass allocation")?;
                    match allocation.as_str() {
                        "INSTANCE" => class_value = None,
                        "CLASS" => class_value = Some(Rc::new(RefCell::new(Value::Unbound))),
                        _ => {
                            return Err(Self::invalid(
                                "defclass allocation must be :instance or :class",
                                option[1].span,
                            ));
                        }
                    }
                }
                "ACCESSOR" | "READER" => {
                    let accessor_name =
                        Self::variable_name(&option[1], "defclass accessor must be a symbol")?;
                    readers.push((unqualified_name(&accessor_name), slot_name.clone()));
                }
                "WRITER" => {
                    let writer_name =
                        Self::variable_name(&option[1], "defclass writer must be a symbol")?;
                    writers.push((unqualified_name(&writer_name), slot_name.clone()));
                }
                "TYPE" => type_form = Some(option[1].clone()),
                "DOCUMENTATION" => {
                    let FormKind::String(value) = &option[1].kind else {
                        return Err(Self::invalid(
                            "defclass slot documentation must be a string",
                            option[1].span,
                        ));
                    };
                    documentation = Some(value.clone());
                }
                _ => {
                    return Err(Self::invalid(
                        "unsupported defclass slot option",
                        option[0].span,
                    ));
                }
            }
        }
        Ok(DefclassSlotRegistration {
            slot: ClassSlot {
                name: slot_name,
                documentation,
                initargs,
                init_form,
                type_form,
                class_value,
            },
            readers,
            writers,
        })
    }
}
