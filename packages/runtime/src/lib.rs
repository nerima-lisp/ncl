//! Evaluation, compilation, loading, and registration orchestration.
use std::collections::{BTreeMap, HashMap};
use std::ptr::NonNull;
mod builtin_trampoline;
mod compile;
#[cfg(test)]
mod constant_tests;
mod constants;
mod error;
mod eval_builtins;
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
mod support;
#[cfg(test)]
mod tests_file_environment;
#[cfg(test)]
mod tests_function_call_drain;
mod time;
mod uninterned_symbols;
pub use error::RuntimeError;
pub use function_call::RuntimeFunctionCaller;
pub use native_error::NativeCondition;
use native_error::native_failure;
use ncl_compiler_front::{FormExpander, MacroRegistry, lower_toplevel_for_eval};
use ncl_object::{
    Function, HeapRootedWord, Instance, ObjectError, Runtime as ObjectRuntime, ThreadContext, Word,
    function_code, make_simple_vector, slot_ref, symbol_name,
};
use ncl_printer::{PrintOptions, StringSink, write};
use ncl_sys::{
    CodeObjectMetadata, CodePtr, RootToken, SafepointMap, SourceLocation, alloc_code,
    invoke_entry_with_function, publish_code, write_code,
};
pub use optimization::{OptimizationMode, RuntimeOptions};
use support::{NativeAbi, NativeInvocation, RuntimeMacroCaller};
/// A running NCL instance and its published native code.
#[derive(Debug)]
pub struct Runtime {
    options: RuntimeOptions,
    context: ThreadContext,
    code: Vec<CodePtr>,
    undefined_function_stub: usize,
    functions: BTreeMap<u32, PublishedFunction>,
    /// Published entry addresses retain their own rooted `CODE` objects.
    entry_codes: BTreeMap<usize, (Box<Word>, RootToken)>, // check-added-lines: allow(word-table) every code Word has its own root token
    rooted_functions: Vec<(Box<Word>, RootToken)>,
    pending_undefined_function_condition: Option<HeapRootedWord>,
    object: ObjectRuntime,
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
        let object = ObjectRuntime::new()?;
        let mut context = ThreadContext::new();
        context.register(&object)?;
        // Every builtin that `ncl_stdlib::register_all` is about to register
        // needs a real native `ENTRY` the moment it is created, so the
        // trampoline must be published and installed first.
        let (builtin_trampoline, undefined_function_stub) =
            builtin_trampoline::install(&object, &mut context)?;
        let undefined_function_stub_address = undefined_function_stub.address();
        ncl_stdlib::register_all(&mut context, &object)?;
        file_environment::register(&mut context, &object)?;
        time::register_builtins(&mut context, &object)?;
        eval_builtins::register(&mut context, &object)?;
        builtin_trampoline::install_arith_fast(&object)?;
        nonlocal::register_control_builtins(&mut context, &object)?;
        object.set_load_port(Box::new(load::RuntimeLoadPort));
        load::register_builtin(&mut context, &object)?;
        Ok(Self {
            options,
            object,
            context,
            code: vec![builtin_trampoline, undefined_function_stub],
            undefined_function_stub: undefined_function_stub_address,
            functions: BTreeMap::new(),
            entry_codes: BTreeMap::new(),
            rooted_functions: Vec::new(),
            pending_undefined_function_condition: None,
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
        self.context
            .set_condition_handler_invoker(function_call::invoke_condition_handler);
        let evaluator = std::ptr::from_mut(self).cast();
        self.context.set_evaluator_runtime(evaluator);
        let result = load::source_forms(self, source);
        self.clear_pending_undefined_function_condition();
        self.context.clear_evaluator_runtime();
        result
    }
    /// Compile and execute a source string through the native pipeline.
    ///
    ///
    /// # Errors
    /// Returns reader, front-end, lowering, or native execution errors.
    pub fn compile(&mut self, source: &str) -> Result<Word, RuntimeError> {
        self.context
            .set_condition_handler_invoker(function_call::invoke_condition_handler);
        let evaluator = std::ptr::from_mut(self).cast();
        self.context.set_evaluator_runtime(evaluator);
        let result = compile::source(self, source);
        self.clear_pending_undefined_function_condition();
        self.context.clear_evaluator_runtime();
        result
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
        self.context
            .set_condition_handler_invoker(function_call::invoke_condition_handler);
        let evaluator = std::ptr::from_mut(self).cast();
        self.context.set_evaluator_runtime(evaluator);
        let result = (|| {
            file_environment::enter_compile_file(self, &pathname, &truename)?;
            let compiled = compile::file(self, &pathname);
            let restored = self.context.leave_progv().map_err(RuntimeError::from);
            match (compiled, restored) {
                (Err(error), _) | (Ok(_), Err(error)) => Err(error),
                (Ok(value), Ok(())) => Ok(value),
            }
        })();
        self.clear_pending_undefined_function_condition();
        self.context.clear_evaluator_runtime();
        result
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
        self.context
            .set_condition_handler_invoker(function_call::invoke_condition_handler);
        let evaluator = std::ptr::from_mut(self).cast();
        self.context.set_evaluator_runtime(evaluator);
        let result = load::file(self, path.as_ref());
        self.clear_pending_undefined_function_condition();
        self.context.clear_evaluator_runtime();
        result
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
        self.pending_undefined_function_condition
            .as_ref()
            .map(HeapRootedWord::get)
    }
    fn clear_pending_undefined_function_condition(&mut self) {
        self.pending_undefined_function_condition = None;
    }
    fn compile_form_with_values(&mut self, form: Word) -> Result<(Word, Vec<Word>), RuntimeError> {
        let registry = MacroRegistry::new();
        let mut caller = RuntimeMacroCaller {
            entry_codes: &self.entry_codes,
        };
        let mut expander = FormExpander::new(&mut self.context, &self.object, &registry);
        expander.set_caller(&mut caller);
        let expr = expander.expand(form)?;
        let lowered = lower_toplevel_for_eval(&expr)?;
        let mut module = ncl_opt::Module {
            functions: std::iter::once(lowered.entry)
                .chain(lowered.nested)
                .collect(),
        };
        module.verify().map_err(|error| {
            RuntimeError::Native(format!(
                "IR verification before optimization failed: {error}"
            ))
        })?;
        if self.options.optimization == OptimizationMode::Default {
            let mut passes = ncl_opt::PassManager::new();
            passes.add_default_optimization_pipeline();
            let result = if self.options.opt_diagnostics {
                passes.run_with_observer(&mut module, |event| eprintln!("{event}"))
            } else {
                passes.run(&mut module)
            };
            result.map_err(|error| RuntimeError::Native(error.to_string()))?;
        }
        module.functions.sort_by_key(|function| function.id);
        let entry = module.functions.first().cloned().ok_or_else(|| {
            RuntimeError::Native("optimization removed entry function".to_owned())
        })?;
        let mut compiled = Vec::with_capacity(module.functions.len());
        for function in &module.functions {
            compiled.push((function, self.compile_native(function)?));
        }
        for (function, (code, metadata)) in &compiled {
            let entry = code.address().saturating_add(metadata.entry_offset);
            self.functions
                .insert(function.id.0, PublishedFunction { entry });
        }
        let mut uninterned_symbols = HashMap::new();
        for (function, (code, metadata)) in compiled.iter().skip(1) {
            let entry = code.address().saturating_add(metadata.entry_offset);
            let (_function_object, entry_code) =
                self.make_function_object(function, &(code, metadata), &mut uninterned_symbols)?;
            self.root_entry_code(entry, entry_code);
        }
        let (entry_code, entry_metadata) = &compiled
            .first()
            .ok_or_else(|| RuntimeError::Native("compiled module is empty".to_owned()))?
            .1;
        let (entry_function, entry_constants) = self.make_function_object(
            &entry,
            &(entry_code, entry_metadata),
            &mut uninterned_symbols,
        )?;
        let entry_address = entry_code
            .address()
            .saturating_add(entry_metadata.entry_offset);
        self.root_entry_code(entry_address, entry_constants);
        let result = self.invoke_compiled(entry_code, entry_metadata, entry_function)?;
        for (_, (code, _)) in compiled {
            self.code.push(code);
        }
        Ok(result)
    }
    fn compile_native(
        &mut self,
        function: &ncl_ir::Function,
    ) -> Result<(CodePtr, CodeObjectMetadata), RuntimeError> {
        let abi = NativeAbi {
            object: &self.object,
            functions: &self.functions,
            undefined_function_stub: self.undefined_function_stub,
        };
        let compiled = if cfg!(target_arch = "aarch64") {
            ncl_codegen::compile_function_aarch64_preserving_multiple_values(function, &abi)
        } else {
            ncl_codegen::compile_function_x86_64_preserving_multiple_values(function, &abi)
        }
        .map_err(|error| RuntimeError::Native(error.to_string()))?;
        let mut code = alloc_code(compiled.code.len())
            .map_err(|error| RuntimeError::Native(format!("{error:?}")))?;
        write_code(&mut code, 0, &compiled.code)
            .map_err(|error| RuntimeError::Native(format!("{error:?}")))?;
        publish_code(&mut code).map_err(|error| RuntimeError::Native(format!("{error:?}")))?;
        let maps = support::encode_maps(&compiled.safepoint_maps)?;
        let safepoint_map = SafepointMap::decode(&maps, compiled.safepoint_maps.len())
            .map_err(|error| RuntimeError::Native(error.to_owned()))?;
        let metadata = CodeObjectMetadata {
            entry_offset: usize::try_from(compiled.entry_offset)
                .map_err(|_| RuntimeError::Native("entry offset is too large".to_owned()))?,
            size: compiled.code.len(),
            frame_words: u16::try_from(compiled.frame_size / 8)
                .map_err(|_| RuntimeError::Native("frame is too large".to_owned()))?,
            function_name: function.name.clone(),
            source_locations: Vec::<SourceLocation>::new(),
            constant_slots: Vec::new(),
            safepoint_map,
            debug_table: Vec::new(),
        };
        if !compiled.safepoint_maps.is_empty() {
            self.context
                .thread_access()
                .register_code(&code, metadata.clone())
                .map_err(|error| RuntimeError::Native(format!("{error:?}")))?;
        }
        Ok((code, metadata))
    }
    fn make_function_object(
        &mut self,
        function: &ncl_ir::Function,
        compiled: &(&CodePtr, &CodeObjectMetadata),
        uninterned_symbols: &mut HashMap<u32, HeapRootedWord>,
    ) -> Result<(Word, Word), RuntimeError> {
        let entry = compiled.0.address().saturating_add(compiled.1.entry_offset);
        let constants = self.make_constants(function, uninterned_symbols)?;
        let code_object = ncl_object::make_code_object(
            &mut self.context,
            &self.object,
            entry,
            compiled.1.size,
            constants,
            Word::NIL,
            Word::NIL,
        )?;
        let function_object = ncl_object::make_simple_fun(
            &mut self.context,
            &self.object,
            entry,
            Word::NIL,
            Word::NIL,
            code_object,
        )?;
        let mut rooted = Box::new(function_object.as_word());
        let token = ncl_object::push_root(&mut self.context, &mut rooted);
        self.rooted_functions.push((rooted, token));
        Ok((function_object.as_word(), code_object.as_word()))
    }
    fn root_entry_code(&mut self, entry: usize, code: Word) {
        let mut rooted = Box::new(code);
        let token = ncl_object::push_root(&mut self.context, &mut rooted);
        self.entry_codes.insert(entry, (rooted, token));
    }
    fn make_constants(
        &mut self,
        function: &ncl_ir::Function,
        uninterned_symbols: &mut HashMap<u32, HeapRootedWord>,
    ) -> Result<Word, RuntimeError> {
        let mut values = Vec::with_capacity(function.constants.len());
        let mut roots = Vec::with_capacity(function.constants.len());
        for constant in &function.constants {
            let value = if let ncl_ir::Constant::FunctionEntry(id) = constant {
                let entry = self
                    .functions
                    .get(&id.0)
                    .ok_or_else(|| {
                        RuntimeError::Native(format!("function entry {} is unavailable", id.0))
                    })?
                    .entry;
                Word::from_bits(u64::try_from(entry).map_err(|_| {
                    RuntimeError::Native("function entry does not fit a machine word".to_owned())
                })?)
            } else {
                constants::resolve_constant(
                    &mut self.context,
                    &self.object,
                    constant,
                    &values,
                    uninterned_symbols,
                )?
            };
            values.push(value);
            let value = values
                .last_mut()
                .ok_or_else(|| RuntimeError::Native("constant table append failed".to_owned()))?;
            roots.push(ncl_object::push_root(&mut self.context, value));
        }
        let result =
            make_simple_vector(&mut self.context, &self.object, &values).map_err(Into::into);
        for token in roots.into_iter().rev() {
            let _ = ncl_object::pop_root(&mut self.context, token);
        }
        result
    }
    fn invoke_compiled(
        &mut self,
        code: &CodePtr,
        metadata: &CodeObjectMetadata,
        function: Word,
    ) -> Result<(Word, Vec<Word>), RuntimeError> {
        let _ = self.context.take_pending_runtime_errors();
        let code_object = function_code(&self.context, Function::from_word(function))?;
        let entry_codes = &self.entry_codes;
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
        let result = invoke_entry_with_function(
            code,
            metadata.entry_offset,
            thread.as_ptr(),
            function.bits(),
            0,
            [0; 4],
            0,
        );
        let _ = ncl_sys::replace_native_context(thread, previous);
        let ((value, value_count), _) = (result, native_context);
        let pending_errors = context.take_pending_runtime_errors();
        if let Some(error) = pending_errors.native {
            return Err(native_failure(error));
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
            if matches!(error, ObjectError::UndefinedFunction)
                && let Some(condition) = context.take_pending_condition()
            {
                let condition = HeapRootedWord::new(context, condition)?;
                #[allow(clippy::option_if_let_else)]
                let name = match slot_ref(context, Instance::from_word(condition.get()), 0)
                    .and_then(|name| symbol_name(context, name))
                    .and_then(|name| {
                        ncl_compiler_front::form::word_string(context, name)
                            .map_err(|_| ObjectError::Layout)
                    }) {
                    Ok(name) => name,
                    Err(_) => "<unknown>".to_owned(),
                };
                self.pending_undefined_function_condition = Some(condition);
                return Err(RuntimeError::UndefinedFunction { name });
            }
            return Err(error.into());
        }
        if context.take_non_local_exit() {
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
        for (_, (_, token)) in std::mem::take(&mut self.entry_codes) {
            let _ = ncl_object::pop_root(&mut self.context, token);
        }
        for (_, token) in self.rooted_functions.drain(..).rev() {
            let _ = ncl_object::pop_root(&mut self.context, token);
        }
    }
}
#[cfg(test)]
mod tests_runtime;
#[cfg(test)]
mod tests_uninterned_symbols;
