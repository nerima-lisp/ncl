//! Evaluation, compilation, loading, and registration orchestration.
use std::collections::{BTreeMap, HashMap};
use std::ptr::NonNull;
use std::rc::Rc;
mod builtin_trampoline;
mod compile;
#[cfg(test)]
mod constant_tests;
mod constants;
mod error;
mod eval_builtins;
mod evaluator;
mod evalwhen;
mod file_environment;
mod function_call;
pub(crate) mod load;
mod local_macro;
#[cfg(test)]
mod makeclosure_tests;
mod native_error;
mod nonlocal;
mod optimization;
pub(crate) mod signal;
mod support;
#[cfg(test)]
mod tests_file_environment;
#[cfg(test)]
mod tests_function_call_drain;
mod time;
mod uninterned_symbols;
pub use error::RuntimeError;
use evaluator::EvaluatorService;
pub use function_call::RuntimeFunctionCaller;
pub use native_error::NativeCondition;
use native_error::native_failure;
use ncl_compiler_front::{FormExpander, MacroRegistry, lower_toplevel_for_eval};
use ncl_object::{
    Function, ObjectError, Runtime as ObjectRuntime, ThreadContext, Word, function_code,
};
use ncl_printer::{PrintOptions, StringSink, write};
use ncl_sys::{CodeObjectMetadata, CodePtr, invoke_entry_with_function};
pub use optimization::{OptimizationMode, RuntimeOptions};
use support::{NativeInvocation, RuntimeMacroCaller};
/// A running NCL instance and its published native code.
#[derive(Debug)]
pub struct Runtime {
    options: RuntimeOptions,
    pub(crate) pass_runner: Option<optimization::StoredPassRunner>,
    context: ThreadContext,
    evaluator: EvaluatorService,

    /// The legacy test invocation references this handle; it shares service state.
    #[cfg(test)]
    entry_codes: EvaluatorService,

    object: Rc<ObjectRuntime>,
}
#[derive(Debug)]
pub(crate) struct PublishedFunction {
    pub(crate) entry: usize,
}
impl Runtime {
    /// Create a runtime and register the standard library exactly once.
    ///
    /// # Errors
    /// Returns an initialization error from the object or standard-library layers.
    pub fn new() -> Result<Self, RuntimeError> {
        Self::with_options(RuntimeOptions::default())
    }
    /// Create a runtime with explicit optimization options.
    ///
    /// # Errors
    /// Returns an initialization error from the object or standard-library layers.
    pub fn with_options(options: RuntimeOptions) -> Result<Self, RuntimeError> {
        let object = Rc::new(ObjectRuntime::new()?);
        let mut context = ThreadContext::new();
        context.register(&object)?;
        // Every builtin that `ncl_stdlib::register_all` is about to register
        // needs a real native `ENTRY` the moment it is created, so the
        // trampoline must be published and installed first.
        let (builtin_trampoline, undefined_function_stub) =
            builtin_trampoline::install(&object, &mut context)?;
        let undefined_function_stub_address = undefined_function_stub.address();
        ncl_object::register(&mut context, &object)?;
        ncl_stdlib::register_all(&mut context, &object)?;
        file_environment::register(&mut context, &object)?;
        time::register_builtins(&mut context, &object)?;
        eval_builtins::register(&mut context, &object)?;
        builtin_trampoline::install_arith_fast(&object)?;
        nonlocal::register_control_builtins(&mut context, &object)?;
        object.set_load_port(Box::new(load::RuntimeLoadPort));
        load::register_builtin(&mut context, &object)?;
        let evaluator = EvaluatorService::new(
            vec![builtin_trampoline, undefined_function_stub],
            undefined_function_stub_address,
        );
        #[cfg(test)]
        let entry_codes = evaluator.clone();
        Ok(Self {
            options,
            pass_runner: None,
            context,
            evaluator,
            #[cfg(test)]
            entry_codes,
            object,
        })
    }
    /// Enables or disables collection before each allocation.
    pub const fn set_gc_stress(&mut self, on: bool) {
        self.context.set_gc_stress(on);
    }
    /// Enables or disables strict forwarding checks.
    pub fn set_strict_forwarding(&self, on: bool) {
        self.context.set_strict_forwarding(on);
    }
    /// Evaluate source by compiling it to native code and invoking the entry.
    ///
    /// # Errors
    /// Returns a reader, front-end, lowering, or native publication error.
    pub fn eval(&mut self, source: &str) -> Result<Word, RuntimeError> {
        self.with_evaluator_access(|runtime| load::source_forms(runtime, source))
    }
    /// Compile and execute a source string through the native pipeline.
    ///
    /// # Errors
    /// Returns reader, front-end, lowering, or native execution errors.
    pub fn compile(&mut self, source: &str) -> Result<Word, RuntimeError> {
        self.with_evaluator_access(|runtime| compile::source(runtime, source))
    }
    /// Compile and execute all forms in a source file.
    ///
    /// # Errors
    /// Returns a file, reader, front-end, lowering, or native execution error.
    pub fn compile_file(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<Word, RuntimeError> {
        let path = path.as_ref();
        let pathname = file_environment::absolute_path(path)?;
        let truename = std::fs::canonicalize(&pathname).map_err(|error| RuntimeError::Io {
            path: pathname.display().to_string(),
            error,
        })?;
        self.with_evaluator_access(|runtime| {
            file_environment::enter_compile_file(runtime, &pathname, &truename)?;
            let compiled = compile::file(runtime, &pathname);
            let restored = runtime.context.leave_progv().map_err(RuntimeError::from);
            match (compiled, restored) {
                (Err(error), _) | (Ok(_), Err(error)) => Err(error),
                (Ok(value), Ok(())) => Ok(value),
            }
        })
    }
    /// Load and execute all forms in a source string.
    ///
    /// # Errors
    /// Returns reader, front-end, lowering, or native execution errors.
    pub fn load(&mut self, source: &str) -> Result<Word, RuntimeError> {
        self.eval(source)
    }
    /// Load and execute all forms in a source file.
    ///
    /// # Errors
    /// Returns a file, reader, front-end, lowering, or native execution error.
    pub fn load_file(&mut self, path: impl AsRef<std::path::Path>) -> Result<Word, RuntimeError> {
        self.with_evaluator_access(|runtime| load::file(runtime, path.as_ref()))
    }
    pub(crate) fn eval_form(&mut self, form: Word) -> Result<Word, RuntimeError> {
        self.eval_form_with_values(form).map(|(value, _)| value)
    }
    pub(crate) fn eval_form_with_values(
        &mut self,
        form: Word,
    ) -> Result<(Word, Vec<Word>), RuntimeError> {
        self.compile_form_with_values(form)
    }
    pub(crate) fn pending_undefined_function_condition(&self) -> Option<Word> {
        self.evaluator.pending_undefined_function_condition()
    }
    fn clear_pending_undefined_function_condition(&self) {
        self.evaluator.clear_pending_undefined_function_condition();
    }
    fn compile_form_with_values(&mut self, form: Word) -> Result<(Word, Vec<Word>), RuntimeError> {
        let registry = MacroRegistry::new();
        let mut caller = RuntimeMacroCaller::new(self.evaluator.clone());
        let mut expander = FormExpander::new(&mut self.context, &self.object, &registry);
        expander.set_caller(&mut caller);
        let expr = expander.expand(form)?;
        let lowered = lower_toplevel_for_eval(&expr)?;
        let mut module = ncl_opt::Module {
            functions: std::iter::once(lowered.entry)
                .chain(lowered.nested)
                .collect(),
        };
        let options = self.options;
        let mut default_runner = optimization::default_pass_runner(options);
        let pass_runner: &mut optimization::PassRunner = match self.pass_runner.as_mut() {
            Some(runner) => runner.0.as_mut(),
            None => &mut default_runner,
        };
        optimization::verify_and_optimize(&mut module, options, pass_runner)?;
        module.functions.sort_by_key(|function| function.id);
        let (entry, nested_functions) = module.functions.split_first().ok_or_else(|| {
            RuntimeError::Native("optimization removed entry function".to_owned())
        })?;
        let entry_native = self.compile_native(entry)?;
        let mut compiled = Vec::with_capacity(nested_functions.len());
        for function in nested_functions {
            compiled.push((function, self.compile_native(function)?));
        }
        let mut pending_functions = BTreeMap::new();
        pending_functions.insert(
            entry.id.0,
            PublishedFunction {
                entry: entry_native.entry_address()?,
            },
        );
        for (function, native) in &compiled {
            pending_functions.insert(
                function.id.0,
                PublishedFunction {
                    entry: native.entry_address()?,
                },
            );
        }
        let mut uninterned_symbols = HashMap::new();
        let mut literal_objects = HashMap::new();
        let mut pending_entry_codes = Vec::with_capacity(module.functions.len());
        let mut function_roots = Vec::with_capacity(module.functions.len());
        for (function, native) in &compiled {
            let entry = native.entry_address()?;
            let (function_object, entry_code) = self.make_function_object(
                function,
                native,
                &pending_functions,
                &mut uninterned_symbols,
                &mut literal_objects,
            )?;
            function_roots.push(function_object);
            pending_entry_codes.push((entry, entry_code));
        }
        let (entry_function, entry_constants) = self.make_function_object(
            entry,
            &entry_native,
            &pending_functions,
            &mut uninterned_symbols,
            &mut literal_objects,
        )?;
        let entry_function_word = entry_function.get();
        function_roots.push(entry_function);
        let entry_address = entry_native.entry_address()?;
        let mut published_entry = (entry, entry_native.into_metadata()?);
        let mut published = compiled
            .into_iter()
            .map(|(function, native)| native.into_metadata().map(|metadata| (function, metadata)))
            .collect::<Result<Vec<_>, _>>()?;
        self.publish_native_with_entry(&mut published_entry, &mut published)?;
        self.evaluator
            .state
            .functions
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend(pending_functions);
        for function in function_roots {
            self.evaluator
                .root_function(&mut self.context, function.get());
        }
        pending_entry_codes.push((entry_address, entry_constants));
        for (entry, code) in pending_entry_codes {
            self.evaluator
                .root_entry_code(&mut self.context, entry, code.get());
        }
        let result = self.invoke_compiled(
            &published_entry.1.0,
            &published_entry.1.1,
            entry_function_word,
        );
        self.evaluator.push_code(published_entry.1.0);
        for (_, (code, _)) in published {
            self.evaluator.push_code(code);
        }
        result
    }
    fn invoke_compiled(
        &mut self,
        code: &CodePtr,
        metadata: &CodeObjectMetadata,
        function: Word,
    ) -> Result<(Word, Vec<Word>), RuntimeError> {
        let code_object = function_code(&self.context, Function::from_word(function))?;
        let entry_codes = &self.evaluator;
        let context = &mut self.context;
        let thread = context.thread_access().as_non_null();
        let mut native_context = NativeInvocation {
            object: &self.object,
            context,
            code: code_object,
            entry_codes,
        };
        let previous = ncl_sys::replace_native_context(
            thread,
            Some(NonNull::from(&mut native_context).cast()),
        );
        if previous.is_none() {
            // Nested invocations must retain pending errors for their caller.
            let _ = native_context.context.take_pending_runtime_errors();
        }
        let result = invoke_entry_with_function(
            code,
            metadata.entry_offset,
            thread.as_ptr(),
            function.bits(),
            0,
            [0; 4],
            0,
        );
        let pending_errors = native_context.context.take_pending_runtime_errors();
        let native_runtime_error = match pending_errors.native {
            Some(error) => {
                match native_error::signal_pending_native_error(
                    native_context.context,
                    native_context.object,
                    error,
                ) {
                    Some(Ok(())) => None,
                    Some(Err(error)) => Some(error.into()),
                    None => Some(native_failure(error)),
                }
            }
            None => None,
        };
        let _ = ncl_sys::replace_native_context(thread, previous);
        let ((value, value_count), _) = (result, native_context);
        if let Some(error) = native_runtime_error {
            return Err(error);
        }
        if let Some(error) = pending_errors.lisp
            && let Some(converter) = self.object.lisp_error_converter()
        {
            let converted = converter(context, &self.object, error);
            let converted_errors = context.take_pending_runtime_errors();
            let condition = converted?;
            context.set_pending_condition(condition);
            if let Some(error) = converted_errors.native {
                return Err(native_failure(error));
            }
            if let Some(error) = converted_errors.object {
                return Err(error.into());
            }
        }
        if let Some(error) = pending_errors.object {
            let mapped_error = {
                let mut pending = self.evaluator.pending_undefined_function_condition_mut();
                native_error::pending_object_error(context, &self.object, error, &mut pending)?
            };
            return Err(mapped_error);
        }
        if context.take_non_local_exit() {
            // Nested evaluation can pass the exit to its compiled caller.
            if previous.is_some() {
                context.set_non_local_exit(true);
                return Err(ObjectError::NonLocalExit.into());
            }
            return Err(ObjectError::ControlError.into());
        }
        let value_count = usize::try_from(value_count).map_err(|_| ObjectError::Layout)?;
        let values = context
            .thread_access()
            .multiple_values()
            .get(..value_count)
            .ok_or(ObjectError::Layout)?
            .to_vec();
        Ok((Word::from_bits(value), values))
    }
    /// Format a result for the command-line frontend.
    pub fn format_result(&mut self, value: Word) -> String {
        if value == Word::TRUE {
            return "T".to_owned();
        }
        if let Some(number) = value.as_fixnum() {
            return number.to_string();
        }
        let mut sink = StringSink::new();
        let options = PrintOptions::new().with_readably(true);
        if write(&mut self.context, &self.object, value, &mut sink, &options).is_ok() {
            sink.into_string()
        } else {
            format!("0x{:x}", value.bits())
        }
    }
    /// Create a function caller that can invoke this runtime's published code.
    #[must_use]
    pub const fn function_caller(&self) -> RuntimeFunctionCaller {
        RuntimeFunctionCaller
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.evaluator.clear_roots(&mut self.context);
    }
}
#[cfg(test)]
#[path = "tests/condition_conversion.rs"]
mod condition_conversion_tests;
#[cfg(test)]
mod tests_runtime;
#[cfg(test)]
mod tests_uninterned_symbols;
