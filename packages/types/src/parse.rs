//! Object-layer adapter for parsing type specifier forms.

use ncl_object::{
    Handle, Local, ObjectRef, Runtime, Scope, ThreadContext, Word, car, cdr, classify_object,
    simple_vector_length, simple_vector_ref, string_length, string_ref, symbol_name,
};

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

/// Parse a type specifier while resolving registered class metaobjects.
///
/// Class objects are converted to their registered class names, including
/// when they occur inside compound type specifiers.
pub fn parse_type_specifier_with_runtime(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    spec: Word,
) -> Result<TypeSpecifier, TypeError> {
    let form = to_type_form_with_runtime(ctx, runtime, spec)?;
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

fn to_type_form_with_runtime(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    word: Word,
) -> Result<TypeForm, TypeError> {
    let mut scope = Scope::new(ctx);
    let word: Handle<'_, Word> = scope.root(Local::from_word(word));
    let word_value = scope.get(word).as_word();
    if word_value == Word::NIL {
        return Ok(TypeForm::Value(crate::Value::Nil));
    }
    if word_value == Word::TRUE {
        return Ok(TypeForm::Value(crate::Value::True));
    }
    if word_value.is_list() {
        let mut forms = Vec::new();
        let mut rest: Handle<'_, Word> = scope.root(Local::from_word(word_value));
        while scope.get(rest).as_word() != Word::NIL {
            let rest_value = scope.get(rest).as_word();
            let item = car(scope.context(), rest_value)?;
            let item: Handle<'_, Word> = scope.root(Local::from_word(item));
            let item_value = scope.get(item).as_word();
            forms.push(to_type_form_with_runtime(
                scope.context_mut(),
                runtime,
                item_value,
            )?);
            let next = cdr(scope.context(), scope.get(rest).as_word())?;
            rest = scope.root(Local::from_word(next));
        }
        return Ok(TypeForm::List(forms));
    }
    if matches!(
        classify_object(scope.context(), word_value),
        ObjectRef::Symbol(_)
    ) {
        let name = string_to_upper(scope.context(), symbol_name(scope.context(), word_value)?)?;
        if crate::NamedType::from_name(&name).is_none()
            && runtime.class(scope.context_mut(), &name).is_some()
        {
            return Ok(TypeForm::Class(name));
        }
        return Ok(TypeForm::Symbol(name));
    }
    let object_word = scope.get(word).as_word();
    if let Some(name) = registered_class_name(scope.context_mut(), runtime, object_word)? {
        if crate::NamedType::from_name(&name).is_some() {
            return Ok(TypeForm::Symbol(name));
        }
        return Ok(TypeForm::Class(name));
    }
    let object_word = scope.get(word).as_word();
    Ok(TypeForm::Value(from_word(scope.context(), object_word)?))
}

fn registered_class_name(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    word: Word,
) -> Result<Option<String>, TypeError> {
    let mut scope = Scope::new(ctx);
    let object: Handle<'_, Word> = scope.root(Local::from_word(word));
    let object_word = scope.get(object).as_word();
    if !matches!(
        classify_object(scope.context(), object_word),
        ObjectRef::SimpleVector(_)
    ) || simple_vector_length(scope.context(), object_word)? < 3
    {
        return Ok(None);
    }
    let name_word = simple_vector_ref(scope.context(), object_word, 0)?;
    #[allow(clippy::wildcard_enum_match_arm)]
    let name = match classify_object(scope.context(), name_word) {
        ObjectRef::Symbol(_) => {
            string_to_upper(scope.context(), symbol_name(scope.context(), name_word)?)?
        }
        ObjectRef::String(_) => {
            let length = string_length(scope.context(), name_word)?;
            (0..length)
                .map(|index| string_ref(scope.context(), name_word, index))
                .collect::<Result<String, _>>()?
        }
        _ => return Ok(None), // check-added-lines: allow(wildcard) registered class names must be symbols or strings
    };
    let registered = runtime.class(scope.context_mut(), &name);
    if registered == Some(scope.get(object).as_word()) {
        Ok(Some(name))
    } else {
        Ok(None)
    }
}
