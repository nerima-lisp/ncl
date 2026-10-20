//! Fast native entries for compiled builtin call sites emitted as
//! `OpKind::Builtin` by `packages/compiler/front/src/lower/expr.rs`.
//!
//! These addresses are recorded in the runtime's builtin-address table
//! (`ncl_object::Runtime::register_builtin_address`) *in addition to*, not
//! instead of, the ordinary generic `ENTRY` every builtin function object
//! gets from `builtin_trampoline::install`. That split matters: the function
//! object's `ENTRY` slot is what indirect calls (`funcall`, `apply`, or a
//! compiled call sites without a matching fast builtin) jump through, so it
//! must stay the safe, arity-checked generic trampoline. Only matching direct
//! builtin call paths read this module's addresses.
//!
//! Each entry inlines a fixnum tag check and falls back to the same generic
//! `ncl_object::Runtime::call_builtin` dispatch `funcall`/`apply` already use
//! (`ncl-lib-numbers`'s `arithmetic::{add,sub,mul}`/`comparison::less`) for
//! any non-fixnum operand or fixnum overflow, so bignum promotion and mixed
//! float/ratio arithmetic behave identically whether or not the fast path
//! fires.
use std::ptr::NonNull;

use ncl_object::{
    BuiltinIdentifier, BuiltinName, BuiltinPackage, FunctionObject, Package,
    Runtime as ObjectRuntime, Word, push_heap_root, symbol_function,
};
use ncl_sys::{Thread, with_native_context};

use crate::RuntimeError;
use crate::builtin_trampoline::{record_boundary_error, unregister_heap_roots};
use crate::support::NativeInvocation;

/// Call the generic Common Lisp `name` builtin with two rooted arguments,
/// returning `Word::NIL` (with a pending condition recorded on `context`)
/// when the native context is unavailable or the call itself fails.
fn call_generic_binary(thread: NonNull<Thread>, name: &str, left: Word, right: Word) -> Word {
    let Some(result) = with_native_context(thread, |invocation: &mut NativeInvocation<'_>| {
        let context = &mut *invocation.context;
        let object = invocation.object;
        let mut arguments = [left, right];
        let argument_tokens = arguments
            .iter_mut()
            .map(|word| push_heap_root(object, word))
            .collect::<Vec<_>>();
        let result = (|| {
            let Some(package) = object.find_package(context, "COMMON-LISP") else {
                context.set_pending(ncl_object::ObjectError::Layout);
                return Word::NIL.bits();
            };
            let Ok((symbol, _)) = Package::from_word(package).intern(context, object, name) else {
                context.set_pending(ncl_object::ObjectError::Layout);
                return Word::NIL.bits();
            };
            let Ok(function_word) = symbol_function(context, symbol) else {
                context.set_pending(ncl_object::ObjectError::Unbound);
                return Word::NIL.bits();
            };
            let Ok(function) = FunctionObject::try_from(function_word) else {
                context.set_pending(ncl_object::ObjectError::Unbound);
                return Word::NIL.bits();
            };
            match object.call_builtin_from_generated_code(context, function, &arguments) {
                Ok(value) => value.bits(),
                Err(error) => {
                    record_boundary_error(context, error);
                    Word::NIL.bits()
                }
            }
        })();
        let arguments_popped = unregister_heap_roots(object, argument_tokens.into_iter().rev());
        if arguments_popped.is_err() {
            context.set_pending(ncl_object::ObjectError::RootStackCorrupted);
            return Word::NIL.bits();
        }
        result
    }) else {
        return Word::NIL;
    };
    Word::from_bits(result)
}

/// Add two fixnums, falling back to the generic numeric tower on overflow or
/// a non-fixnum operand.
extern "C" fn native_add_fast(thread: NonNull<Thread>, left: Word, right: Word) -> Word {
    if let (Some(x), Some(y)) = (left.as_fixnum(), right.as_fixnum())
        && let Some(sum) = x.checked_add(y)
    {
        let word = Word::fixnum(sum);
        if word.as_fixnum() == Some(sum) {
            return word;
        }
    }
    call_generic_binary(thread, "+", left, right)
}

/// Subtract two fixnums, falling back to the generic numeric tower on
/// overflow or a non-fixnum operand.
extern "C" fn native_sub_fast(thread: NonNull<Thread>, left: Word, right: Word) -> Word {
    if let (Some(x), Some(y)) = (left.as_fixnum(), right.as_fixnum())
        && let Some(difference) = x.checked_sub(y)
    {
        let word = Word::fixnum(difference);
        if word.as_fixnum() == Some(difference) {
            return word;
        }
    }
    call_generic_binary(thread, "-", left, right)
}

/// Increment a fixnum, falling back to generic addition on overflow or a
/// non-fixnum operand.
extern "C" fn native_one_plus_fast(thread: NonNull<Thread>, value: Word) -> Word {
    if let Some(integer) = value.as_fixnum()
        && let Some(sum) = integer.checked_add(1)
    {
        let word = Word::fixnum(sum);
        if word.as_fixnum() == Some(sum) {
            return word;
        }
    }
    call_generic_binary(thread, "+", value, Word::fixnum(1))
}

/// Decrement a fixnum, falling back to generic subtraction on overflow or a
/// non-fixnum operand.
extern "C" fn native_one_minus_fast(thread: NonNull<Thread>, value: Word) -> Word {
    if let Some(integer) = value.as_fixnum()
        && let Some(difference) = integer.checked_sub(1)
    {
        let word = Word::fixnum(difference);
        if word.as_fixnum() == Some(difference) {
            return word;
        }
    }
    call_generic_binary(thread, "-", value, Word::fixnum(1))
}

/// Multiply two fixnums, falling back to the generic numeric tower on
/// overflow or a non-fixnum operand.
extern "C" fn native_mul_fast(thread: NonNull<Thread>, left: Word, right: Word) -> Word {
    if let (Some(x), Some(y)) = (left.as_fixnum(), right.as_fixnum())
        && let Some(product) = x.checked_mul(y)
    {
        let word = Word::fixnum(product);
        if word.as_fixnum() == Some(product) {
            return word;
        }
    }
    call_generic_binary(thread, "*", left, right)
}

/// Compare two fixnums, falling back to the generic numeric tower for a
/// non-fixnum operand (floats, ratios, bignums).
extern "C" fn native_less_fast(thread: NonNull<Thread>, left: Word, right: Word) -> Word {
    if let (Some(x), Some(y)) = (left.as_fixnum(), right.as_fixnum()) {
        return if x < y { Word::TRUE } else { Word::NIL };
    }
    call_generic_binary(thread, "<", left, right)
}

/// Publish the fast entries for `+`/`-`/`*`/`<`/`1+`/`1-` into the
/// runtime's builtin-address table, so the compiler's lowered call sites
/// (`packages/compiler/front/src/lower/expr.rs`) reach them
/// instead of the shared generic trampoline.
///
/// Must run after `ncl_stdlib::register_all` has registered these builtins,
/// since it only augments the existing table entry rather than creating one.
///
/// # Errors
/// Returns an error if an entry's address does not fit the runtime's
/// address representation.
pub fn install(object: &ObjectRuntime) -> Result<(), RuntimeError> {
    let entries: [(&str, u64); 6] = [
        (
            "+",
            ncl_sys::function_address!(native_add_fast)
                .map_err(|error| RuntimeError::Native(error.to_string()))?,
        ),
        (
            "-",
            ncl_sys::function_address!(native_sub_fast)
                .map_err(|error| RuntimeError::Native(error.to_string()))?,
        ),
        (
            "1+",
            ncl_sys::function_address!(native_one_plus_fast)
                .map_err(|error| RuntimeError::Native(error.to_string()))?,
        ),
        (
            "1-",
            ncl_sys::function_address!(native_one_minus_fast)
                .map_err(|error| RuntimeError::Native(error.to_string()))?,
        ),
        (
            "*",
            ncl_sys::function_address!(native_mul_fast)
                .map_err(|error| RuntimeError::Native(error.to_string()))?,
        ),
        (
            "<",
            ncl_sys::function_address!(native_less_fast)
                .map_err(|error| RuntimeError::Native(error.to_string()))?,
        ),
    ];
    for (name, address) in entries {
        let address = usize::try_from(address)
            .map_err(|_| RuntimeError::Native("fast entry address overflow".to_owned()))?;
        object.register_builtin_address(
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            address,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ptr::NonNull;

    use super::{native_one_minus_fast, native_one_plus_fast};
    use ncl_object::Word;
    use ncl_sys::Thread;

    #[test]
    fn unary_entries_adjust_fixnums() {
        let mut context = Thread::new();
        let thread = NonNull::from(&mut context);

        assert_eq!(
            native_one_plus_fast(thread, Word::fixnum(41)),
            Word::fixnum(42)
        );
        assert_eq!(
            native_one_minus_fast(thread, Word::fixnum(-41)),
            Word::fixnum(-42)
        );
    }
}
