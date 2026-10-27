use std::collections::BTreeMap;
use std::ptr::NonNull;

use ncl_codegen::{AbiError, RuntimeAbi, RuntimeFunction};
use ncl_compiler_front::{FrontError, MacroCaller, SymbolRef};
use ncl_object::{
    BuiltinIdentifier, CodeObject, Function, FunctionObject, HeapRootedWord, ObjectError, Package,
    Runtime as ObjectRuntime, ThreadContext, Word, cdr, function_code,
    function_entry, make_closure, make_cons, make_value_cell, symbol_function,
};
use ncl_sys::{Thread, invoke_entry_with_function_address, replace_native_context, thread_layout};

#[cfg(test)]
use ncl_sys::StorageCondition;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{PublishedFunction, RuntimeError};

mod write_barrier;

pub struct NativeInvocation<'a> {
    pub(crate) object: &'a ObjectRuntime,
    pub(crate) context: &'a mut ThreadContext,
    pub(crate) code: CodeObject,
    /// Native entry address -> that function's own `CODE` object, so
    /// `MakeClosure` can attach the callee's code (and constants table)
    /// rather than always inheriting the currently-executing function's.
    pub(crate) entry_codes: &'a BTreeMap<usize, (Box<Word>, ncl_sys::RootToken)>, // check-added-lines: allow(word-table) borrowed rooted code entries
}

#[cfg(test)]
static MAKE_CLOSURE_FAILURE_THREAD: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
pub struct MakeClosureFailureGuard(usize);

#[cfg(test)]
impl Drop for MakeClosureFailureGuard {
    fn drop(&mut self) {
        let _ = MAKE_CLOSURE_FAILURE_THREAD.compare_exchange(
            self.0,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    }
}

#[cfg(test)]
pub fn inject_make_closure_capacity_failure(
    context: &mut ThreadContext,
) -> MakeClosureFailureGuard {
    let thread = context.thread_access().identity();
    MAKE_CLOSURE_FAILURE_THREAD.store(thread, Ordering::SeqCst);
    MakeClosureFailureGuard(thread)
}

#[cfg(test)]
fn take_make_closure_capacity_failure(thread: NonNull<Thread>, capture_count: usize) -> bool {
    if capture_count == 0 {
        return false;
    }
    MAKE_CLOSURE_FAILURE_THREAD
        .compare_exchange(
            thread.as_ptr().addr(),
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        )
        .is_ok()
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
            ncl_codegen::ContextField::RuntimeErrorFlags => layout.runtime_error_flags,
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
            RuntimeFunction::WriteBarrier => {
                ncl_sys::function_address!(write_barrier::native_write_barrier)
                    .map_err(|error| RuntimeError::Native(error.to_string()))
                    .map_err(|_| AbiError::UnsupportedRuntimeFunction(function))
            }
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

impl ncl_types::DeftypeExpanderCaller for RuntimeMacroCaller<'_> {
    fn call_expander(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &ObjectRuntime,
        function: Word,
        whole_form: Word,
    ) -> Result<Word, ObjectError> {
        let function = FunctionObject::try_from(function)?;
        call_macro_function(ctx, runtime, function, &[whole_form], self.entry_codes)
    }
}

impl MacroCaller for RuntimeMacroCaller<'_> {
    fn call_macro(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &ObjectRuntime,
        name: &ncl_compiler_front::SymbolRef,
        form: Word,
    ) -> Result<Word, ncl_compiler_front::FrontError> {
        let form_root =
            HeapRootedWord::new(ctx, form).map_err(|error| expansion(name, &error.to_string()))?;
        let package = name
            .package_name()
            .ok_or_else(|| expansion(name, "uninterned macro has no function cell"))?;
        let package_word = runtime
            .find_package(ctx, package)
            .ok_or_else(|| expansion(name, "macro package is not present"))?;
        let (symbol, _) = Package::from_word(package_word)
            .intern(ctx, runtime, &name.name)
            .map_err(|error| expansion(name, &error.to_string()))?;
        let function = symbol_function(ctx, symbol).map_err(|e| expansion(name, &e.to_string()))?;
        let function_root = HeapRootedWord::new(ctx, function)
            .map_err(|error| expansion(name, &error.to_string()))?;
        let form = if name.package_name() == Some("COMMON-LISP") {
            let package = runtime
                .find_package(ctx, "COMMON-LISP")
                .ok_or_else(|| expansion(name, "macro package is not present"))?;
            let (head, _) = Package::from_word(package)
                .intern(ctx, runtime, &name.name)
                .map_err(|error| expansion(name, &error.to_string()))?;
            let tail =
                cdr(ctx, form_root.get()).map_err(|error| expansion(name, &error.to_string()))?;
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
            form_root.get()
        };
        let function = FunctionObject::try_from(function_root.get())
            .map_err(|error| expansion(name, &error.to_string()))?;
        if runtime.builtin_descriptor(ctx, function).is_some() {
            return runtime
                .call_builtin(ctx, function, &[form])
                .map_err(|error| {
                    let detail = pending_simple_error_message(ctx, runtime, || error.to_string());
                    expansion(name, &detail)
                });
        }
        // A compiled macro function takes exactly one argument: the whole
        // call form. `defmacro` (see `ncl-lib-macros::defining`) compiles
        // the macro lambda list into a single-argument lambda that
        // destructures `(cdr whole)` itself, so it can support `&whole`,
        // `&environment`, `&body`, and nested patterns without help from
        // the front end's ordinary-lambda-list lowering. Passing one
        // argument here also avoids the register/rest-pointer marshalling
        // in `call_macro_function` for arbitrarily wide argument lists,
        // which is what a `&rest`/`&body` macro previously could trip
        // non-deterministically under GC pressure.
        let result = call_macro_function(ctx, runtime, function, &[form], self.entry_codes);
        result.map_err(|error| {
            let detail = pending_simple_error_message(ctx, runtime, || error.to_string());
            expansion(name, &detail)
        })
    }

    fn expand_type_specifier(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &ObjectRuntime,
        specifier: Word,
    ) -> Result<Word, FrontError> {
        ncl_types::expand_deftype_specifier_with(ctx, runtime, specifier, self).map_err(|error| {
            FrontError::MacroExpansion {
                name: SymbolRef::interned("COMMON-LISP", "DEFTYPE"),
                detail: error.to_string(),
            }
        })
    }

    fn call_local_macro(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &ObjectRuntime,
        definition: &ncl_compiler_front::ast::LocalMacro,
        form: Word,
    ) -> Result<Word, ncl_compiler_front::FrontError> {
        crate::local_macro::call_local_macro(ctx, runtime, self.entry_codes, definition, form)
    }
}

pub fn call_macro_function(
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
        let thread = ctx.thread_access().as_non_null();
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
        let pending_errors = ctx.take_pending_runtime_errors();
        if pending_errors.native.is_some() {
            return Err(ObjectError::Layout);
        }
        if let Some(error) = pending_errors.lisp
            && let Some(converter) = runtime.lisp_error_converter()
        {
            let converted = converter(ctx, runtime, error);
            let converted_errors = ctx.take_pending_runtime_errors();
            let condition = converted?;
            ctx.set_pending_condition(condition);
            if converted_errors.native.is_some() {
                return Err(ObjectError::Layout);
            }
            if let Some(error) = converted_errors.object {
                return Err(error);
            }
        }
        if let Some(error) = pending_errors.object {
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

fn pending_simple_error_message(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    fallback: impl FnOnce() -> String,
) -> String {
    let Some(condition) = ctx.take_pending_condition() else {
        return fallback();
    };
    let simple_error = ncl_conditions::ConditionIdentifier::SimpleError.class(ctx, runtime);
    if ncl_conditions::condition_class_of(ctx, condition).ok() == simple_error
        && let Some(report) = ncl_conditions::condition_report(ctx, condition)
    {
        return report;
    }
    ctx.set_pending_condition(condition);
    fallback()
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
        #[cfg(test)]
        let result = if take_make_closure_capacity_failure(thread, capture_count) {
            // Keep the allocation error path reproducible without changing production behavior.
            Err(ObjectError::Storage(StorageCondition::CapacityExceeded))
        } else {
            make_closure(
                ctx,
                invocation.object,
                entry,
                Word::NIL,
                Word::NIL,
                code,
                &captures,
            )
        };
        #[cfg(not(test))]
        let result = make_closure(
            ctx,
            invocation.object,
            entry,
            Word::NIL,
            Word::NIL,
            code,
            &captures,
        );
        match result {
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

pub fn encode_maps(maps: &[ncl_codegen::SafepointMap]) -> Result<Vec<u8>, RuntimeError> {
    maps.iter().try_fold(Vec::new(), |mut bytes, map| {
        bytes.extend(
            map.encode()
                .map_err(|error| RuntimeError::Native(error.to_string()))?,
        );
        Ok(bytes)
    })
}
