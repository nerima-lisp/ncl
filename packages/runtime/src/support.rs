use std::collections::BTreeMap;
use std::ptr::NonNull;

use ncl_codegen::{AbiError, RuntimeAbi, RuntimeFunction};
use ncl_compiler_front::MacroCaller;
use ncl_object::{
    BuiltinIdentifier, CodeObject, FunctionObject, ObjectError, Package, Runtime as ObjectRuntime,
    ThreadContext, Word, cdr, make_closure, make_cons, make_double, make_string, symbol_function,
};
use ncl_sys::{Thread, thread_layout};

use crate::{PublishedFunction, RuntimeError};

pub(crate) struct NativeInvocation<'a> {
    pub(crate) object: &'a ObjectRuntime,
    pub(crate) context: &'a mut ThreadContext,
    pub(crate) code: CodeObject,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeAbi<'a> {
    pub(crate) object: &'a ObjectRuntime,
    pub(crate) functions: &'a BTreeMap<u32, PublishedFunction>,
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
            ncl_codegen::ContextField::Pending
            | ncl_codegen::ContextField::MultipleValueCount
            | ncl_codegen::ContextField::Handler
            | ncl_codegen::ContextField::Cleanup
            | ncl_codegen::ContextField::Catch => {
                return Err(AbiError::UnsupportedContextField(field));
            }
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
            RuntimeFunction::AllocateSlow
            | RuntimeFunction::Unwind
            | RuntimeFunction::Builtin
            | RuntimeFunction::ConstantTable
            | RuntimeFunction::EnterCatch
            | RuntimeFunction::EnterUnwindProtect
            | RuntimeFunction::EnterProgv
            | RuntimeFunction::LeaveCatch
            | RuntimeFunction::LeaveUnwindProtect
            | RuntimeFunction::LeaveProgv => Err(AbiError::UnsupportedRuntimeFunction(function)),
        }
    }

    fn constant_word_named(&self, name: ncl_codegen::ConstantName<'_>) -> Option<i64> {
        let id = name.as_str().strip_prefix("function-entry:")?.parse().ok()?;
        let function = self.functions.get(&id)?;
        i64::try_from(function.entry).ok()
    }
}

pub(crate) struct RuntimeMacroCaller;

impl MacroCaller for RuntimeMacroCaller {
    fn call_macro(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &ObjectRuntime,
        name: &ncl_compiler_front::SymbolRef,
        form: Word,
    ) -> Result<Word, ncl_compiler_front::FrontError> {
        let package = name.package_name().ok_or_else(|| expansion(name, "uninterned macro has no function cell"))?;
        let package_word = runtime.find_package(ctx, package).ok_or_else(|| expansion(name, "macro package is not present"))?;
        let (symbol, _) = Package::from_word(package_word)
            .intern(ctx, runtime, &name.name)
            .map_err(|error| expansion(name, &error.to_string()))?;
        let function = symbol_function(ctx, symbol).map_err(|error| expansion(name, &error.to_string()))?;
        let function = FunctionObject::try_from(function).map_err(|error| expansion(name, &error.to_string()))?;
        let form = if name.package_name() == Some("COMMON-LISP") {
            let package = runtime.find_package(ctx, "COMMON-LISP").ok_or_else(|| expansion(name, "macro package is not present"))?;
            let (head, _) = Package::from_word(package)
                .intern(ctx, runtime, &name.name)
                .map_err(|error| expansion(name, &error.to_string()))?;
            let tail = cdr(ctx, form).map_err(|error| expansion(name, &error.to_string()))?;
            ncl_object::with_roots(ctx, &[head, tail], |ctx, roots| {
                make_cons(ctx, runtime, **roots.first().ok_or(ObjectError::Layout)?, **roots.get(1).ok_or(ObjectError::Layout)?)
            })
            .map_err(|error| expansion(name, &error.to_string()))?
        } else {
            form
        };
        runtime.call_builtin(ctx, function, &[form]).map_err(|error| expansion(name, &error.to_string()))
    }
}

fn expansion(name: &ncl_compiler_front::SymbolRef, detail: &str) -> ncl_compiler_front::FrontError {
    ncl_compiler_front::FrontError::MacroExpansion { name: name.clone(), detail: detail.to_owned() }
}

pub(crate) extern "C" fn native_make_closure(
    thread: NonNull<Thread>,
    entry: Word,
    capture0: Word,
    capture1: Word,
    capture2: Word,
) -> Word {
    ncl_sys::with_native_context(thread, |invocation: &mut NativeInvocation<'_>| {
        let ctx = &mut *invocation.context;
        let entry = match usize::try_from(entry.bits()) {
            Ok(entry) => entry,
            Err(_) => {
                ctx.set_pending(ObjectError::Layout);
                return Word::NIL;
            }
        };
        match make_closure(ctx, invocation.object, entry, Word::NIL, Word::NIL, invocation.code, &[capture0, capture1, capture2]) {
            Ok(function) => function.as_word(),
            Err(error) => {
                ctx.set_pending(error);
                Word::NIL
            }
        }
    })
    .unwrap_or(Word::NIL)
}

pub(crate) fn resolve_constant(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    constant: &ncl_ir::Constant,
    previous: &[Word],
) -> Result<Word, RuntimeError> {
    ncl_object::with_roots(ctx, previous, |ctx, previous| -> Result<Word, ObjectError> {
        let value = match constant {
            ncl_ir::Constant::Fixnum(value) => Word::fixnum(*value),
            ncl_ir::Constant::Character(value) => Word::character(*value),
            ncl_ir::Constant::Nil => Word::NIL,
            ncl_ir::Constant::T => Word::TRUE,
            // check-added-lines: allow(unbound) IR explicitly represents this sentinel.
            ncl_ir::Constant::Unbound => Word::UNBOUND,
            ncl_ir::Constant::Object(index) => previous.get(usize::try_from(index.0).map_err(|_| ObjectError::Layout)?).map(|value| **value).ok_or(ObjectError::Layout)?,
            ncl_ir::Constant::StringBytes(bytes) => {
                let text = std::str::from_utf8(bytes).map_err(|_| ObjectError::Layout)?;
                make_string(ctx, runtime, &text.chars().collect::<Vec<_>>())?
            }
            ncl_ir::Constant::DoubleFloat(value) => make_double(ctx, runtime, *value)?.as_word(),
            ncl_ir::Constant::SingleFloat(value) => make_double(ctx, runtime, f64::from(*value))?.as_word(),
            ncl_ir::Constant::Symbol { package, name } => {
                let package_word = runtime.find_package(ctx, package).ok_or(ObjectError::Layout)?;
                Package::from_word(package_word).intern(ctx, runtime, name).map(|(symbol, _)| symbol)?
            }
            ncl_ir::Constant::FunctionEntry(_) => Word::NIL,
        };
        Ok(value)
    }).map_err(Into::into)
}

pub(crate) fn encode_maps(maps: &[ncl_codegen::SafepointMap]) -> Result<Vec<u8>, RuntimeError> {
    maps.iter().try_fold(Vec::new(), |mut bytes, map| {
        bytes.extend(map.encode().map_err(|error| RuntimeError::Native(error.to_string()))?);
        Ok(bytes)
    })
}
