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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{TypeForm, to_type_form};
    use ncl_object::{Package, Runtime, ThreadContext, Word, make_cons, make_string};

    #[test]
    fn converts_nil_true_lists_symbols_and_values() {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("thread context");
        let package = runtime.find_package(&ctx, "COMMON-LISP").expect("package");
        let symbol = Package::from_word(package)
            .intern(&mut ctx, &runtime, "mixed-case")
            .expect("symbol")
            .0;
        let string = make_string(&mut ctx, &runtime, &['x']).expect("string");
        let list = make_cons(&mut ctx, &runtime, symbol, Word::NIL).expect("list");

        assert_eq!(
            to_type_form(&mut ctx, Word::NIL),
            Ok(TypeForm::Value(crate::Value::Nil))
        );
        assert_eq!(
            to_type_form(&mut ctx, Word::TRUE),
            Ok(TypeForm::Value(crate::Value::True))
        );
        assert!(
            matches!(to_type_form(&mut ctx, list), Ok(TypeForm::List(items)) if items.len() == 1)
        );
        assert_eq!(
            to_type_form(&mut ctx, symbol),
            Ok(TypeForm::Symbol("MIXED-CASE".into()))
        );
        assert!(
            matches!(to_type_form(&mut ctx, string), Ok(TypeForm::Value(crate::Value::String(value))) if value == "x")
        );
    }
}
