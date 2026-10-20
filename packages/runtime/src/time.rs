use std::sync::OnceLock;
use std::time::Instant;

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError, Package, Parameter,
    Runtime, ThreadContext, Word, set_symbol_constant, set_symbol_value,
};

const INTERNAL_TIME_UNITS_PER_SECOND: i64 = 1_000_000;
const NANOS_PER_INTERNAL_TIME_UNIT: u128 = 1_000;
const MAX_FIXNUM: i64 = i64::MAX >> ncl_sys::FIXNUM_TAG_BITS;
const NO_PARAMETERS: &[Parameter] = &[];

static MONOTONIC_BASE: OnceLock<Instant> = OnceLock::new();

fn internal_time_ticks(elapsed_nanos: u128) -> i64 {
    let elapsed_units = elapsed_nanos / NANOS_PER_INTERNAL_TIME_UNIT;
    i64::try_from(elapsed_units).map_or(MAX_FIXNUM, |ticks| ticks.min(MAX_FIXNUM))
}

fn current_internal_time() -> Word {
    let elapsed_nanos = MONOTONIC_BASE
        .get_or_init(Instant::now)
        .elapsed()
        .as_nanos();
    Word::fixnum(internal_time_ticks(elapsed_nanos))
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "RustBuiltin callbacks require the Result<Word, ObjectError> signature."
)]
fn get_internal_time(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(current_internal_time())
}

pub fn register_builtins(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(NO_PARAMETERS),
        convention: BuiltinConvention::Direct(Arity::exact(0)),
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("GET-INTERNAL-REAL-TIME"),
        ),
        BuiltinImplementation::direct(descriptor, get_internal_time),
    )?;
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("GET-INTERNAL-RUN-TIME"),
        ),
        BuiltinImplementation::direct(descriptor, get_internal_time),
    )?;

    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let (symbol, _) =
        Package::from_word(package).intern(ctx, runtime, "INTERNAL-TIME-UNITS-PER-SECOND")?;
    set_symbol_value(ctx, symbol, Word::fixnum(INTERNAL_TIME_UNITS_PER_SECOND))?;
    set_symbol_constant(ctx, symbol, true)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MAX_FIXNUM, internal_time_ticks};

    #[test]
    fn internal_time_saturates_at_the_largest_positive_fixnum() {
        assert_eq!(internal_time_ticks(0), 0);
        assert_eq!(internal_time_ticks(999), 0);
        assert_eq!(internal_time_ticks(1_000), 1);
        assert_eq!(internal_time_ticks(2_001), 2);
        assert_eq!(internal_time_ticks(u128::MAX), MAX_FIXNUM);
    }

    #[test]
    fn monotonic_clock_values_do_not_decrease() {
        let first = super::current_internal_time().as_fixnum();
        let second = super::current_internal_time().as_fixnum();
        assert!(matches!(
            (first, second),
            (Some(first), Some(second)) if first >= 0 && second >= first
        ));
    }
}
