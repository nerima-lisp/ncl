//! Object-layer adapter for parsing type specifier forms.

use ncl_object::{ObjectRef, ThreadContext, Word, car, cdr, classify_object, symbol_name};

use crate::adapter::from_word;
use crate::text::string_to_upper;
use crate::{TypeError, TypeSpecifier};

use crate::domain::{TypeForm, parse_type_form};

/// Parse a runtime type specifier into a [`TypeSpecifier`].
pub fn parse_type_specifier(
    ctx: &mut ThreadContext,
    spec: Word,
) -> Result<TypeSpecifier, TypeError> {
    let form = to_type_form(ctx, spec)?;
    parse_type_form(form).map_err(|error| {
        if error == TypeError::InvalidForm {
            TypeError::InvalidSpecifier(spec)
        } else {
            error
        }
    })
}

fn to_type_form(ctx: &mut ThreadContext, word: Word) -> Result<TypeForm, TypeError> {
    if word == Word::NIL {
        return Ok(TypeForm::Value(crate::Value::Nil));
    }
    if word == Word::TRUE {
        return Ok(TypeForm::Value(crate::Value::True));
    }
    if word.is_list() {
        let mut forms = Vec::new();
        let mut rest = word;
        while rest != Word::NIL {
            let item = car(ctx, rest)?;
            forms.push(to_type_form(ctx, item)?);
            rest = cdr(ctx, rest)?;
        }
        return Ok(TypeForm::List(forms));
    }
    if matches!(classify_object(ctx, word), ObjectRef::Symbol(_)) {
        return Ok(TypeForm::Symbol(string_to_upper(
            ctx,
            symbol_name(ctx, word)?,
        )?));
    }
    Ok(TypeForm::Value(from_word(ctx, word)?))
}
