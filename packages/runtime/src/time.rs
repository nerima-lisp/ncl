//! Common Lisp internal clock primitives.

use std::ptr::NonNull;
use std::sync::OnceLock;
use std::time::Instant;

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError, Runtime as ObjectRuntime,
    ThreadContext, Word, set_symbol_constant, set_symbol_value,
};
use ncl_sys::Thread;

const INTERNAL_TIME_UNITS_PER_SECOND: i64 = 1_000_000;
static START: OnceLock<Instant> = OnceLock::new();

fn internal_real_time() -> Word {
    let micros = START
        .get_or_init(Instant::now)
        .elapsed()
        .as_micros()
        .try_into()
        .unwrap_or(i64::MAX);
    Word::fixnum(micros)
}

fn internal_real_time_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &ObjectRuntime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(internal_real_time())
}

pub extern "C" fn native_get_internal_real_time(_thread: NonNull<Thread>) -> Word {
    internal_real_time()
}

/// Register the monotonic clock used by `GET-INTERNAL-REAL-TIME`.
pub fn register(ctx: &mut ThreadContext, runtime: &ObjectRuntime) -> Result<(), ObjectError> {
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(&[]),
        convention: BuiltinConvention::Direct(Arity::exact(0)),
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("GET-INTERNAL-REAL-TIME"),
        ),
        BuiltinImplementation::direct(descriptor, internal_real_time_builtin).with_entry(
            ncl_sys::function_address!(native_get_internal_real_time)
                .map_err(|_| ObjectError::Layout)
                .and_then(|address| usize::try_from(address).map_err(|_| ObjectError::Layout))?,
        ),
    )?;
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let (symbol, _) = ncl_object::Package::from_word(package).intern(
        ctx,
        runtime,
        "INTERNAL-TIME-UNITS-PER-SECOND",
    )?;
    set_symbol_constant(ctx, symbol, true)?;
    set_symbol_value(ctx, symbol, Word::fixnum(INTERNAL_TIME_UNITS_PER_SECOND))
}

#[cfg(test)]
mod tests {
    use super::{INTERNAL_TIME_UNITS_PER_SECOND, internal_real_time};

    #[test]
    fn internal_real_time_is_monotonic() {
        let before = internal_real_time().as_fixnum().expect("fixnum time");
        std::thread::sleep(std::time::Duration::from_millis(1));
        let after = internal_real_time().as_fixnum().expect("fixnum time");
        assert!(after > before);
    }

    #[test]
    fn internal_time_units_are_microseconds_per_second() {
        assert_eq!(INTERNAL_TIME_UNITS_PER_SECOND, 1_000_000);
    }
}
