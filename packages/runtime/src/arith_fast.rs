//! Fast native entries for the compiled two-argument `+`/`-`/`*`/`<` call
//! sites `packages/compiler/front/src/lower/expr.rs` emits as `OpKind::Builtin`.
//!
//! These addresses are recorded in the runtime's builtin-address table
//! (`ncl_object::Runtime::register_builtin_address`) *in addition to*, not
//! instead of, the ordinary generic `ENTRY` every builtin function object
//! gets from `builtin_trampoline::install`. That split matters: the function
//! object's `ENTRY` slot is what indirect calls (`funcall`, `apply`, or a
//! compiled call site with any arity other than two) jump through, so it
//! must stay the safe, arity-checked generic trampoline. Only the codegen
//! literal-two-argument fast path reads this module's addresses.
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
    Runtime as ObjectRuntime, Word, pop_heap_root, push_heap_root, symbol_function,
};
use ncl_sys::{Thread, with_native_context};

use crate::RuntimeError;
use crate::builtin_trampoline::record_boundary_error;
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
            match object.call_builtin(context, function, &arguments) {
                Ok(value) => value.bits(),
                Err(error) => {
                    record_boundary_error(context, error);
                    Word::NIL.bits()
                }
            }
        })();
        let arguments_popped = argument_tokens
            .into_iter()
            .rev()
            .all(|token| pop_heap_root(object, token));
        if !arguments_popped {
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

/// Publish the fast two-argument entries for `+`/`-`/`*`/`<` into the
/// runtime's builtin-address table, so the compiler's literal-two-argument
/// call sites (`packages/compiler/front/src/lower/expr.rs`) reach them
/// instead of the shared generic trampoline.
///
/// Must run after `ncl_stdlib::register_all` has registered these builtins,
/// since it only augments the existing table entry rather than creating one.
///
/// # Errors
/// Returns an error if an entry's address does not fit the runtime's
/// address representation.
pub fn install(object: &ObjectRuntime) -> Result<(), RuntimeError> {
    let entries: [(&str, u64); 4] = [
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
