use std::collections::BTreeMap;
use std::ptr::NonNull;

use ncl_codegen::{AbiError, RuntimeAbi, RuntimeFunction};
use ncl_compiler_front::MacroCaller;
use ncl_object::{
    BuiltinIdentifier, CodeObject, Function, FunctionObject, ObjectError, Package,
    Runtime as ObjectRuntime, ThreadContext, Word, cdr, function_code, function_entry,
    make_bignum_from_limbs, make_closure, make_complex, make_cons, make_double, make_ratio,
    make_simple_vector, make_string, make_value_cell, symbol_function,
};
use ncl_sys::{Thread, invoke_entry_with_function_address, replace_native_context, thread_layout};

use crate::{PublishedFunction, RuntimeError};

pub struct NativeInvocation<'a> {
    pub(crate) object: &'a ObjectRuntime,
    pub(crate) context: &'a mut ThreadContext,
    pub(crate) code: CodeObject,
    /// Native entry address -> that function's own `CODE` object, so
    /// `MakeClosure` can attach the callee's code (and constants table)
    /// rather than always inheriting the currently-executing function's.
    pub(crate) entry_codes: &'a BTreeMap<usize, (Box<Word>, ncl_sys::RootToken)>, // check-added-lines: allow(word-table) borrowed rooted code entries
}

#[derive(Clone, Copy, Debug)]
pub struct NativeAbi<'a> {
    pub(crate) object: &'a ObjectRuntime,
    pub(crate) functions: &'a BTreeMap<u32, PublishedFunction>,
    pub(crate) undefined_function_stub: usize,
}

impl RuntimeAbi for NativeAbi<'_> {
    fn builtin_address(&self, identifier: BuiltinIdentifier) -> Result<u64, AbiError> {
        self.object
            .builtin_address(identifier)
            .ok_or(AbiError::MissingBuiltin(identifier))
    }

    fn field_offset(&self, field: ncl_codegen::ContextField) -> Result<i32, AbiError> {
        let layout = thread_layout();
        let offset = match field {
            ncl_codegen::ContextField::TlabBump => layout.tlab_bump,
            ncl_codegen::ContextField::TlabLimit => layout.tlab_limit,
            ncl_codegen::ContextField::SafepointRequest => layout.safepoint_request,
            ncl_codegen::ContextField::MultipleValueArea => layout.mv,
            ncl_codegen::ContextField::Pending => layout.pending,
            ncl_codegen::ContextField::MultipleValueCount => layout.mv_count,
            ncl_codegen::ContextField::Handler => layout.handler,
            ncl_codegen::ContextField::Cleanup => layout.cleanup,
            ncl_codegen::ContextField::Catch => layout.catch,
        };
        i32::try_from(offset).map_err(|_| AbiError::UnsupportedContextField(field))
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
        match function {
            RuntimeFunction::SafepointSlow => ncl_sys::function_address!(ncl_sys::native_safepoint)
                .map_err(|error| RuntimeError::Native(error.to_string()))
                .map_err(|_| AbiError::UnsupportedRuntimeFunction(function)),
            RuntimeFunction::MakeClosure => ncl_sys::function_address!(native_make_closure)
                .map_err(|error| RuntimeError::Native(error.to_string()))
                .map_err(|_| AbiError::UnsupportedRuntimeFunction(function)),
            RuntimeFunction::MakeValueCell => ncl_sys::function_address!(native_make_value_cell)
                .map_err(|error| RuntimeError::Native(error.to_string()))
                .map_err(|_| AbiError::UnsupportedRuntimeFunction(function)),
            RuntimeFunction::EnterCatch => {
                ncl_sys::function_address!(crate::nonlocal::native_enter_catch)
                    .map_err(|error| RuntimeError::Native(error.to_string()))
                    .map_err(|_| AbiError::UnsupportedRuntimeFunction(function))
            }
            RuntimeFunction::LeaveCatch => {
                ncl_sys::function_address!(crate::nonlocal::native_leave_catch)
                    .map_err(|error| RuntimeError::Native(error.to_string()))
                    .map_err(|_| AbiError::UnsupportedRuntimeFunction(function))
            }
            RuntimeFunction::EnterUnwindProtect => {
                ncl_sys::function_address!(crate::nonlocal::native_enter_unwind_protect)
                    .map_err(|error| RuntimeError::Native(error.to_string()))
                    .map_err(|_| AbiError::UnsupportedRuntimeFunction(function))
            }
            RuntimeFunction::LeaveUnwindProtect => {
                ncl_sys::function_address!(crate::nonlocal::native_leave_unwind_protect)
                    .map_err(|error| RuntimeError::Native(error.to_string()))
                    .map_err(|_| AbiError::UnsupportedRuntimeFunction(function))
            }
            RuntimeFunction::EnterProgv => {
                ncl_sys::function_address!(crate::nonlocal::native_enter_progv)
                    .map_err(|error| RuntimeError::Native(error.to_string()))
                    .map_err(|_| AbiError::UnsupportedRuntimeFunction(function))
            }
            RuntimeFunction::LeaveProgv => {
                ncl_sys::function_address!(crate::nonlocal::native_leave_progv)
                    .map_err(|error| RuntimeError::Native(error.to_string()))
                    .map_err(|_| AbiError::UnsupportedRuntimeFunction(function))
            }
            RuntimeFunction::AllocateSlow
            | RuntimeFunction::Unwind
            | RuntimeFunction::Builtin
            | RuntimeFunction::ConstantTable => Err(AbiError::UnsupportedRuntimeFunction(function)),
            RuntimeFunction::UndefinedFunction => {
                Ok(u64::try_from(self.undefined_function_stub)
                    .map_err(|_| AbiError::UnsupportedRuntimeFunction(function))?)
            }
        }
    }

    fn constant_word_named(&self, name: ncl_codegen::ConstantName<'_>) -> Option<i64> {
        let id = name
            .as_str()
            .strip_prefix("function-entry:")?
            .parse()
            .ok()?;
        let function = self.functions.get(&id)?;
        i64::try_from(function.entry).ok()
    }
}

pub struct RuntimeMacroCaller<'a> {
    pub(crate) entry_codes: &'a BTreeMap<usize, (Box<Word>, ncl_sys::RootToken)>, // check-added-lines: allow(word-table) entries are rooted code objects
}

impl MacroCaller for RuntimeMacroCaller<'_> {
    fn call_macro(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &ObjectRuntime,
        name: &ncl_compiler_front::SymbolRef,
        form: Word,
    ) -> Result<Word, ncl_compiler_front::FrontError> {
        let package = name
            .package_name()
            .ok_or_else(|| expansion(name, "uninterned macro has no function cell"))?;
        let package_word = runtime
            .find_package(ctx, package)
            .ok_or_else(|| expansion(name, "macro package is not present"))?;
        let (symbol, _) = Package::from_word(package_word)
            .intern(ctx, runtime, &name.name)
            .map_err(|error| expansion(name, &error.to_string()))?;
        let function =
            symbol_function(ctx, symbol).map_err(|error| expansion(name, &error.to_string()))?;
        let function = FunctionObject::try_from(function)
            .map_err(|error| expansion(name, &error.to_string()))?;
        let form = if name.package_name() == Some("COMMON-LISP") {
            let package = runtime
                .find_package(ctx, "COMMON-LISP")
                .ok_or_else(|| expansion(name, "macro package is not present"))?;
            let (head, _) = Package::from_word(package)
                .intern(ctx, runtime, &name.name)
                .map_err(|error| expansion(name, &error.to_string()))?;
            let tail = cdr(ctx, form).map_err(|error| expansion(name, &error.to_string()))?;
            ncl_object::with_roots(ctx, &[head, tail], |ctx, roots| {
                make_cons(
                    ctx,
                    runtime,
                    **roots.first().ok_or(ObjectError::Layout)?,
                    **roots.get(1).ok_or(ObjectError::Layout)?,
                )
            })
            .map_err(|error| expansion(name, &error.to_string()))?
        } else {
            form
        };
        if runtime.builtin_descriptor(function).is_some() {
            return runtime
                .call_builtin(ctx, function, &[form])
                .map_err(|error| expansion(name, &error.to_string()));
        }
        let arguments = {
            let tail = cdr(ctx, form).map_err(|error| expansion(name, &error.to_string()))?;
            ncl_compiler_front::form::list(ctx, tail)
                .map_err(|error| expansion(name, &error.to_string()))?
        };
        let result = call_macro_function(ctx, runtime, function, &arguments, self.entry_codes);
        result.map_err(|error| expansion(name, &error.to_string()))
    }
}

fn call_macro_function(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    function: FunctionObject,
    arguments: &[Word],
    entry_codes: &BTreeMap<usize, (Box<Word>, ncl_sys::RootToken)>, // check-added-lines: allow(word-table) entries are rooted code objects
) -> Result<Word, ObjectError> {
    ncl_object::with_rooted_slice(ctx, arguments, |ctx, rooted| {
        let function_word = function.as_word();
        let entry = function_entry(ctx, Function::from_word(function_word))?;
        if entry == 0 {
            return Err(ObjectError::Layout);
        }
        let code = function_code(ctx, Function::from_word(function_word))?;
        let mut registers = [0_u64; 4];
        for (register, argument) in rooted.iter().take(4).enumerate() {
            let slot = registers.get_mut(register).ok_or(ObjectError::Layout)?;
            *slot = argument.bits();
        }
        let argument_count = u64::try_from(rooted.len()).map_err(|_| ObjectError::Layout)?;
        let rest = rooted
            .get_mut(4..)
            .filter(|rest| !rest.is_empty())
            .map_or(Ok(0), |rest| {
                u64::try_from(rest.as_mut_ptr().addr()).map_err(|_| ObjectError::Layout)
            })?;
        let thread = NonNull::from(ctx.thread_mut());
        let mut native_context = NativeInvocation {
            object: runtime,
            context: ctx,
            code,
            entry_codes,
        };
        let previous =
            replace_native_context(thread, Some(NonNull::from(&mut native_context).cast()));
        let (result, _) = invoke_entry_with_function_address(
            entry,
            thread.as_ptr(),
            function_word.bits(),
            argument_count,
            registers,
            rest,
        );
        let _restored_context = replace_native_context(thread, previous);
        if ctx.thread_mut().take_native_error().is_some() {
            return Err(ObjectError::Layout);
        }
        if let Some(error) = ctx.take_pending_lisp_error()
            && let Some(converter) = runtime.lisp_error_converter()
        {
            let condition = converter(ctx, runtime, error)?;
            ctx.set_pending_condition(condition);
        }
        if let Some(error) = ctx.take_pending() {
            return Err(error);
        }
        Ok(Word::from_bits(result))
    })
}

fn expansion(name: &ncl_compiler_front::SymbolRef, detail: &str) -> ncl_compiler_front::FrontError {
    ncl_compiler_front::FrontError::MacroExpansion {
        name: name.clone(),
        detail: detail.to_owned(),
    }
}

pub extern "C" fn native_make_closure(
    thread: NonNull<Thread>,
    capture_count: Word,
    entry: Word,
) -> Word {
    ncl_sys::with_native_context(thread, |invocation: &mut NativeInvocation<'_>| {
        let ctx = &mut *invocation.context;
        let Ok(entry) = usize::try_from(entry.bits()) else {
            ctx.set_pending(ObjectError::Layout);
            return Word::NIL;
        };
        let Ok(capture_count) = capture_count
            .as_fixnum()
            .ok_or(ObjectError::Layout)
            .and_then(|count| usize::try_from(count).map_err(|_| ObjectError::Layout))
        else {
            ctx.set_pending(ObjectError::Layout);
            return Word::NIL;
        };
        // A closure over a separately-published top-level function must
        // carry *that* function's own CODE object (and constants table),
        // not the caller's. Only fall back to the caller's CODE object for
        // a nested lambda that was compiled as part of the same function
        // (and therefore shares its constants table).
        let code = invocation
            .entry_codes
            .get(&entry)
            .map_or(invocation.code, |(word, _)| CodeObject::from_word(**word));
        let captures = vec![Word::NIL; capture_count];
        match make_closure(
            ctx,
            invocation.object,
            entry,
            Word::NIL,
            Word::NIL,
            code,
            &captures,
        ) {
            Ok(function) => function.as_word(),
            Err(error) => {
                ctx.set_pending(error);
                Word::NIL
            }
        }
    })
    .unwrap_or(Word::NIL)
}

pub extern "C" fn native_make_value_cell(thread: NonNull<Thread>, value: Word) -> Word {
    ncl_sys::with_native_context(thread, |invocation: &mut NativeInvocation<'_>| {
        match make_value_cell(invocation.context, invocation.object, value) {
            Ok(cell) => cell,
            Err(error) => {
                invocation.context.set_pending(error);
                Word::NIL
            }
        }
    })
    .unwrap_or(Word::NIL)
}

pub fn resolve_constant(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    constant: &ncl_ir::Constant,
    previous: &[Word],
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

pub fn encode_maps(maps: &[ncl_codegen::SafepointMap]) -> Result<Vec<u8>, RuntimeError> {
    maps.iter().try_fold(Vec::new(), |mut bytes, map| {
        bytes.extend(
            map.encode()
                .map_err(|error| RuntimeError::Native(error.to_string()))?,
        );
        Ok(bytes)
    })
}
