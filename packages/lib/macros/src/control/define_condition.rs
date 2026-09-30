//! `DEFINE-CONDITION` (C1), expanded onto
//! `NCL-EXT::DEFINE-CONDITION-CLASS`/`NCL-EXT::CONDITION-SLOT-REF`.
//!
//! Phase 1 simplification: a slot's positional index is its position among
//! its *own* class's directly declared slots only: slots are not
//! accumulated from a user-defined parent the way ANSI's inherited-slot
//! model requires. A `DEFINE-CONDITION` with no slots of its own (the common
//! case: adding a `:report` to a built-in supertype) is unaffected; one that
//! both inherits slots from a user-defined parent *and* declares its own
//! needs the parent's slot count folded in, which this does not do.

use super::handler_bind::symbol_named;
use super::handler_case::{cons_list, form, intern, word_handle};
use ncl_object::{Handle, Local, ObjectError, Runtime, Scope, ThreadContext, Word};

type Result<T = Word> = std::result::Result<T, ObjectError>;

struct Slot<'ctx> {
    initarg: Option<Handle<'ctx>>,
    initform: Option<Handle<'ctx>>,
    reader: Option<Handle<'ctx>>,
    accessor: Option<Handle<'ctx>>,
}

fn parse_slot<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    spec_word: Word,
) -> std::result::Result<Slot<'ctx>, ObjectError> {
    let parts = scope.list_to_handle_vec(Local::from_word(spec_word))?;
    let parts_slice = parts.as_slice();
    if parts_slice.is_empty() {
        // A bare slot-name symbol, e.g. `foo` rather than `(foo ...)`.
        return Ok(Slot {
            initarg: None,
            initform: None,
            reader: None,
            accessor: None,
        });
    }
    let mut slot = Slot {
        initarg: None,
        initform: None,
        reader: None,
        accessor: None,
    };
    let mut index = 1;
    while index + 1 < parts_slice.len() {
        let Some(key) = parts_slice.get(index).copied() else {
            break;
        };
        let Some(value) = parts_slice.get(index + 1).copied() else {
            break;
        };
        let key_word = scope.get(key).as_word();
        if symbol_named(scope.context(), key_word, "INITARG") {
            let keyword_word = scope.get(value).as_word();
            let keyword_name = ncl_object::symbol_name(scope.context(), keyword_word)?;
            let text = string_of(scope.context(), keyword_name)?;
            slot.initarg = Some(intern(scope, runtime, &format!("KEYWORD::{text}"))?);
        } else if symbol_named(scope.context(), key_word, "INITFORM") {
            slot.initform = Some(value);
        } else if symbol_named(scope.context(), key_word, "READER") {
            slot.reader = Some(value);
        } else if symbol_named(scope.context(), key_word, "ACCESSOR") {
            slot.accessor = Some(value);
        }
        index += 2;
    }
    let _ = runtime;
    Ok(slot)
}

fn string_of(ctx: &ThreadContext, word: Word) -> std::result::Result<String, ObjectError> {
    let length = ncl_object::string_length(ctx, word)?;
    let mut text = String::with_capacity(length);
    for i in 0..length {
        text.push(ncl_object::string_ref(ctx, word, i)?);
    }
    Ok(text)
}

/// Expand `(define-condition name (parent*) (slot-spec*) option*)`.
pub(super) fn expand(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    let mut scope = Scope::new(ctx);
    let name = word_handle(&mut scope, *values.first().ok_or(ObjectError::TypeError)?);
    let parents_word = *values.get(1).ok_or(ObjectError::TypeError)?;
    let slot_specs_word = *values.get(2).ok_or(ObjectError::TypeError)?;
    let option_words: Vec<Local<'_>> = values
        .get(3..)
        .unwrap_or(&[])
        .iter()
        .copied()
        .map(Local::from_word)
        .collect();
    let option_handles = scope.root_many(&option_words);

    let quoted_name = form(&mut scope, runtime, "QUOTE", &[name])?;
    let parents = word_handle(&mut scope, parents_word);
    let quoted_parents = form(&mut scope, runtime, "QUOTE", &[parents])?;

    let slot_words = scope.list_to_handle_vec(Local::from_word(slot_specs_word))?;
    let mut slots = Vec::with_capacity(slot_words.len());
    for index in 0..slot_words.len() {
        let handle = *slot_words
            .as_slice()
            .get(index)
            .ok_or(ObjectError::TypeError)?;
        let word = scope.get(handle).as_word();
        slots.push(parse_slot(&mut scope, runtime, word)?);
    }

    let mut spec_forms = Vec::with_capacity(slots.len());
    let mut extra_forms: Vec<Handle<'_>> = Vec::new();
    for (slot_index, slot) in slots.iter().enumerate() {
        let initarg_form = match slot.initarg {
            Some(keyword) => form(&mut scope, runtime, "QUOTE", &[keyword])?,
            None => word_handle(&mut scope, Word::NIL),
        };
        let initform_thunk = match slot.initform {
            Some(initform) => {
                let empty_lambda_list = word_handle(&mut scope, Word::NIL);
                form(
                    &mut scope,
                    runtime,
                    "LAMBDA",
                    &[empty_lambda_list, initform],
                )?
            }
            None => word_handle(&mut scope, Word::NIL),
        };
        let pair = form(&mut scope, runtime, "CONS", &[initarg_form, initform_thunk])?;
        spec_forms.push(pair);

        let index_literal = word_handle(
            &mut scope,
            Word::fixnum(i64::try_from(slot_index).unwrap_or(0)),
        );
        for accessor in [slot.reader, slot.accessor].into_iter().flatten() {
            let instance_parameter = super::handler_case::fresh(&mut scope, runtime)?;
            let lambda_list = cons_list(&mut scope, runtime, &[instance_parameter])?;
            let body = form(
                &mut scope,
                runtime,
                "NCL-EXT::CONDITION-SLOT-REF",
                &[instance_parameter, index_literal],
            )?;
            let defun_form = form(&mut scope, runtime, "DEFUN", &[accessor, lambda_list, body])?;
            extra_forms.push(defun_form);
        }
    }
    let list_symbol_form = {
        let list_operator = intern(&mut scope, runtime, "LIST")?;
        let rest = cons_list(&mut scope, runtime, &spec_forms)?;
        scope.make_cons(runtime, list_operator, rest)?
    };

    let mut report_form = word_handle(&mut scope, Word::NIL);
    for option_handle in option_handles.as_slice() {
        let option_word = scope.get(*option_handle).as_word();
        let parts = scope.list_to_handle_vec(Local::from_word(option_word))?;
        let parts_slice = parts.as_slice();
        let Some(head) = parts_slice.first().copied() else {
            continue;
        };
        let head_word = scope.get(head).as_word();
        if symbol_named(scope.context(), head_word, "REPORT")
            && let Some(value) = parts_slice.get(1).copied()
        {
            report_form = value;
        }
    }

    let define_call = form(
        &mut scope,
        runtime,
        "NCL-EXT::DEFINE-CONDITION-CLASS",
        &[quoted_name, quoted_parents, list_symbol_form, report_form],
    )?;
    let mut all_forms = vec![define_call];
    all_forms.extend(extra_forms);
    all_forms.push(quoted_name);
    let progn_form = form(&mut scope, runtime, "PROGN", &all_forms)?;
    Ok(scope.get(progn_form).as_word())
}
