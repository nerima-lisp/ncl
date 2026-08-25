use ncl_syntax::{Form, FormKind};

use crate::value::{ConditionInitarg, ConditionSlot};
use crate::{Runtime, RuntimeError};

use super::invalid;

pub(super) fn parse_slot(runtime: &Runtime, form: &Form) -> Result<ConditionSlot, RuntimeError> {
    let (name_form, options) = match &form.kind {
        FormKind::Atom(_) => (form, &[][..]),
        FormKind::List(items) if !items.is_empty() => (&items[0], &items[1..]),
        _ => {
            return Err(invalid(
                "condition slot must be a symbol or non-empty list",
                form.span,
            ));
        }
    };
    let name =
        runtime.definition_name_from_form(name_form, "condition slot name must be a symbol")?;
    let mut slot = ConditionSlot {
        name,
        initarg: None,
        init_form: None,
        readers: Vec::new(),
        writers: Vec::new(),
    };
    if !options.len().is_multiple_of(2) {
        return Err(invalid(
            "condition slot options must be keyword/value pairs",
            form.span,
        ));
    }
    for pair in options.chunks_exact(2) {
        let option_name = runtime
            .definition_name_from_form(&pair[0], "condition slot option name must be a symbol")?;
        match option_name.as_str() {
            "INITARG" => {
                slot.initarg =
                    nil_or_initarg(runtime, &pair[1], "condition initarg must be a symbol")?;
            }
            "INITFORM" => slot.init_form = Some(pair[1].clone()),
            "READER" => {
                if let Some(reader) =
                    nil_or_name(runtime, &pair[1], "condition reader must be a symbol")?
                {
                    slot.readers.push(reader);
                }
            }
            "WRITER" => {
                if let Some(writer) =
                    nil_or_name(runtime, &pair[1], "condition writer must be a symbol")?
                {
                    slot.writers.push(writer);
                }
            }
            _ => {
                return Err(invalid(
                    format!("unknown condition slot option :{option_name}"),
                    pair[0].span,
                ));
            }
        }
    }
    Ok(slot)
}

fn nil_or_name(
    runtime: &Runtime,
    form: &Form,
    context: &str,
) -> Result<Option<String>, RuntimeError> {
    let (name, escaped) = runtime.definition_name_info_from_form(form, context)?;
    if !escaped && name == "NIL" {
        Ok(None)
    } else {
        Ok(Some(name))
    }
}

fn nil_or_initarg(
    runtime: &Runtime,
    form: &Form,
    context: &str,
) -> Result<Option<ConditionInitarg>, RuntimeError> {
    let (name, escaped) = runtime.definition_name_info_from_form(form, context)?;
    if !escaped && name == "NIL" {
        Ok(None)
    } else {
        Ok(Some(ConditionInitarg { name, escaped }))
    }
}
