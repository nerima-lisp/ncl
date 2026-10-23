use std::collections::HashMap;

use ncl_object::{
    HeapRootedWord, ObjectError, Package, Runtime as ObjectRuntime, ThreadContext, Word,
    make_bignum_from_limbs, make_complex, make_cons, make_double, make_ratio, make_simple_vector,
    make_string,
};

use crate::RuntimeError;

pub fn resolve_constant(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    constant: &ncl_ir::Constant,
    previous: &[Word],
    uninterned_symbols: &mut HashMap<u32, HeapRootedWord>,
) -> Result<Word, RuntimeError> {
    ncl_object::with_roots(
        ctx,
        previous,
        |ctx, previous| -> Result<Word, ObjectError> {
            let resolved = |index: &ncl_ir::ConstantIndex| -> Result<Word, ObjectError> {
                previous
                    .get(usize::try_from(index.0).map_err(|_| ObjectError::Layout)?)
                    .map(|value| **value)
                    .ok_or(ObjectError::Layout)
            };
            let value = match constant {
                ncl_ir::Constant::Fixnum(value) => Word::fixnum(*value),
                ncl_ir::Constant::Character(value) => Word::character(*value),
                // `FunctionEntry` is resolved by `Runtime::make_constants` before it
                // ever reaches this helper; NIL is a harmless fallback here.
                ncl_ir::Constant::Nil | ncl_ir::Constant::FunctionEntry(_) => Word::NIL,
                ncl_ir::Constant::T => Word::TRUE,
                // check-added-lines: allow(unbound) IR explicitly represents this sentinel.
                ncl_ir::Constant::Unbound => Word::UNBOUND,
                ncl_ir::Constant::Object(index) => resolved(index)?,
                ncl_ir::Constant::StringBytes(bytes) => {
                    let text = std::str::from_utf8(bytes).map_err(|_| ObjectError::Layout)?;
                    make_string(ctx, runtime, &text.chars().collect::<Vec<_>>())?
                }
                ncl_ir::Constant::Structure { kind, elements } => {
                    let values = elements
                        .iter()
                        .map(resolved)
                        .collect::<Result<Vec<_>, ObjectError>>()?;
                    match kind {
                        ncl_ir::StructureKind::Cons => {
                            if values.len() != 2 {
                                return Err(ObjectError::Layout);
                            }
                            let Some(car) = values.first() else {
                                return Err(ObjectError::Layout);
                            };
                            let Some(cdr) = values.get(1) else {
                                return Err(ObjectError::Layout);
                            };
                            make_cons(ctx, runtime, *car, *cdr)?
                        }
                        ncl_ir::StructureKind::SimpleVector => {
                            make_simple_vector(ctx, runtime, &values)?
                        }
                    }
                }
                ncl_ir::Constant::DoubleFloat(value) => {
                    make_double(ctx, runtime, *value)?.as_word()
                }
                ncl_ir::Constant::SingleFloat(value) => {
                    make_double(ctx, runtime, f64::from(*value))?.as_word()
                }
                ncl_ir::Constant::Symbol { package, name } => {
                    let package_word = runtime
                        .find_package(ctx, package)
                        .ok_or(ObjectError::Layout)?;
                    Package::from_word(package_word)
                        .intern(ctx, runtime, name)
                        .map(|(symbol, _)| symbol)?
                }
                ncl_ir::Constant::UninternedSymbol { identity, name } => {
                    crate::uninterned_symbols::resolve(
                        ctx,
                        runtime,
                        *identity,
                        name,
                        uninterned_symbols,
                    )?
                }
                ncl_ir::Constant::Bignum { negative, limbs } => {
                    make_bignum_from_limbs(ctx, runtime, *negative, limbs)?.as_word()
                }
                ncl_ir::Constant::Ratio {
                    numerator,
                    denominator,
                } => make_ratio(ctx, runtime, resolved(numerator)?, resolved(denominator)?)?
                    .as_word(),
                ncl_ir::Constant::Complex { real, imaginary } => {
                    make_complex(ctx, runtime, resolved(real)?, resolved(imaginary)?)?.as_word()
                }
            };
            Ok(value)
        },
    )
    .map_err(Into::into)
}
