//! Evaluation, compilation, loading, and registration orchestration.

use std::collections::BTreeMap;
use std::ptr::NonNull;

mod compile;
mod function_call;
mod load;
mod native_error;
mod support;

pub use function_call::RuntimeFunctionCaller;

use ncl_compiler_front::{FormExpander, MacroRegistry, lower_toplevel};
use ncl_object::{
    Function, ObjectError, Runtime as ObjectRuntime, ThreadContext, Word, function_code,
    make_simple_vector,
};
use ncl_sys::{
    CodeObjectMetadata, CodePtr, NativeError, RootToken, SafepointMap, SourceLocation, alloc_code,
    invoke_entry_with_function, publish_code, register_code, write_code,
};

pub use native_error::NativeCondition;
use native_error::native_failure;
use support::{NativeAbi, NativeInvocation, RuntimeMacroCaller};

/// Errors raised while setting up or executing one compilation unit.
#[derive(Debug)]
pub enum RuntimeError {
    /// File-system failure while reading a source unit.
    Io {
        /// Path of the source file that could not be read.
        path: String,
        /// Underlying file-system error.
        error: std::io::Error,
    },
    /// Object-layer failure.
    Object(ObjectError),
    /// Reader failure.
    Read(ncl_reader::ReadError),
    /// Front-end failure.
    Front(ncl_compiler_front::FrontError),
    /// Lowering failure.
    Lower(ncl_compiler_front::LowerError),
    /// Code generation or executable-memory failure.
    Native(String),
    /// A direct native entry failed and was returned through its typed side channel.
    NativeFailure {
        /// The original typed native failure.
        error: NativeError,
        /// The object or Lisp condition category exposed to the runtime.
        condition: NativeCondition,
    },
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, error } => write!(f, "cannot read {path}: {error}"),
            Self::Object(error) => write!(f, "object error: {error}"),
            Self::Read(error) => write!(f, "read error: {error:?}"),
            Self::Front(error) => write!(f, "front-end error: {error:?}"),
            Self::Lower(error) => write!(f, "lowering error: {error:?}"),
            Self::Native(error) => write!(f, "native error: {error}"),
            Self::NativeFailure { error, condition } => {
                write!(f, "native failure {error:?}: {condition:?}")
            }
        }
    }
}

impl std::error::Error for RuntimeError {}
impl From<ObjectError> for RuntimeError {
    fn from(value: ObjectError) -> Self {
        Self::Object(value)
    }
}
impl From<ncl_reader::ReadError> for RuntimeError {
    fn from(value: ncl_reader::ReadError) -> Self {
        Self::Read(value)
    }
}
impl From<std::io::Error> for RuntimeError {
    fn from(value: std::io::Error) -> Self {
        Self::Io {
            path: "<source>".to_owned(),
            error: value,
        }
    }
}
impl From<ncl_compiler_front::FrontError> for RuntimeError {
    fn from(value: ncl_compiler_front::FrontError) -> Self {
        Self::Front(value)
    }
}
impl From<ncl_compiler_front::LowerError> for RuntimeError {
    fn from(value: ncl_compiler_front::LowerError) -> Self {
        Self::Lower(value)
    }
}

/// A running NCL instance and its published native code.
#[derive(Debug)]
pub struct Runtime {
    context: ThreadContext,
    code: Vec<CodePtr>,
    functions: BTreeMap<u32, PublishedFunction>,
    rooted_functions: Vec<(Box<Word>, RootToken)>,
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
        let object = ObjectRuntime::new()?;
        let mut context = ThreadContext::new();
        context.register(&object)?;
        ncl_stdlib::register_all(&mut context, &object)?;
        Ok(Self {
            object,
            context,
            code: Vec::new(),
            functions: BTreeMap::new(),
            rooted_functions: Vec::new(),
        })
    }

    /// Evaluate source by compiling it to native code and invoking the entry.
    ///
    /// # Errors
    /// Returns a reader, front-end, lowering, or native publication error.
    pub fn eval(&mut self, source: &str) -> Result<Word, RuntimeError> {
        let forms = compile::read_forms(&mut self.context, &self.object, source)?;
        let mut result = Word::NIL;
        for form in forms {
            result = self.compile_form(form)?;
        }
        Ok(result)
    }

    /// Compile and execute a source string through the native pipeline.
    ///
    /// The current native pipeline publishes code as it compiles it, so this
    /// is intentionally equivalent to [`Self::eval`].
    ///
    /// # Errors
    /// Returns reader, front-end, lowering, or native execution errors.
    pub fn compile(&mut self, source: &str) -> Result<Word, RuntimeError> {
        compile::source(self, source)
    }

    /// Compile and execute all forms in a source file.
    ///
    /// # Errors
    /// Returns a file, reader, front-end, lowering, or native execution error.
    pub fn compile_file(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<Word, RuntimeError> {
        compile::file(self, path.as_ref())
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
        load::file(self, path.as_ref())
    }

    fn compile_form(&mut self, form: Word) -> Result<Word, RuntimeError> {
        let registry = MacroRegistry::new();
        let mut caller = RuntimeMacroCaller;
        let mut expander = FormExpander::new(&mut self.context, &self.object, &registry);
        expander.set_caller(&mut caller);
        let expr = expander.expand(form)?;
        let lowered = lower_toplevel(&expr)?;
        let mut module = ncl_opt::Module {
            functions: std::iter::once(lowered.entry)
                .chain(lowered.nested)
                .collect(),
        };
        let mut passes = ncl_opt::PassManager::new();
        passes.add_function_pass(ncl_opt::InlineDirectCalls::default());
        passes
            .run(&mut module)
            .map_err(|error| RuntimeError::Native(error.to_string()))?;
        module.functions.sort_by_key(|function| function.id);
        let entry = module.functions.first().cloned().ok_or_else(|| {
            RuntimeError::Native("optimization removed entry function".to_owned())
        })?;
        for function in module.functions.into_iter().skip(1) {
            self.publish_function(function)?;
        }
        let compiled = self.compile_native(&entry)?;
        let entry_metadata = compiled.1.clone();
        let entry_address = compiled
            .0
            .address()
            .saturating_add(entry_metadata.entry_offset);
        let entry_function = self.make_function_object(&entry, &(&compiled.0, &compiled.1))?;
        self.functions.insert(
            entry.id.0,
            PublishedFunction {
                entry: entry_address,
            },
        );
        let value = self.invoke_compiled(&compiled.0, &compiled.1, entry_function)?;
        self.code.push(compiled.0);
        Ok(value)
    }

    fn compile_native(
        &mut self,
        function: &ncl_ir::Function,
    ) -> Result<(CodePtr, CodeObjectMetadata), RuntimeError> {
        let abi = NativeAbi {
            object: &self.object,
            functions: &self.functions,
        };
        let compiled = if cfg!(target_arch = "aarch64") {
            ncl_codegen::compile_function_aarch64(function, &abi)
        } else {
            ncl_codegen::compile_function_x86_64(function, &abi)
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
            register_code(self.context.thread_mut(), &code, metadata.clone())
                .map_err(|error| RuntimeError::Native(format!("{error:?}")))?;
        }
        Ok((code, metadata))
    }

    fn make_function_object(
        &mut self,
        function: &ncl_ir::Function,
        compiled: &(&CodePtr, &CodeObjectMetadata),
    ) -> Result<Word, RuntimeError> {
        let entry = compiled.0.address().saturating_add(compiled.1.entry_offset);
        let constants = self.make_constants(function)?;
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
        Ok(function_object.as_word())
    }

    fn make_constants(&mut self, function: &ncl_ir::Function) -> Result<Word, RuntimeError> {
        let mut values = Vec::with_capacity(function.constants.len());
        for constant in &function.constants {
            let value = support::resolve_constant(&mut self.context, &self.object, constant, &values)?;
            values.push(value);
        }
        make_simple_vector(&mut self.context, &self.object, &values).map_err(Into::into)
    }

    fn publish_function(&mut self, function: ncl_ir::Function) -> Result<(), RuntimeError> {
        let id = function.id;
        let (code, metadata) = self.compile_native(&function)?;
        let entry = code.address().saturating_add(metadata.entry_offset);
        self.make_function_object(&function, &(&code, &metadata))?;
        self.functions.insert(id.0, PublishedFunction { entry });
        self.code.push(code);
        Ok(())
    }

    fn invoke_compiled(
        &mut self,
        code: &CodePtr,
        metadata: &CodeObjectMetadata,
        function: Word,
    ) -> Result<Word, RuntimeError> {
        let code_object = function_code(&self.context, Function::from_word(function))?;
        let context = &mut self.context;
        context.thread_mut().take_native_error();
        let thread = NonNull::from(context.thread_mut());
        let mut native_context = NativeInvocation {
            object: &self.object,
            context,
            code: code_object,
        };
        let previous = ncl_sys::replace_native_context(
            thread,
            Some(NonNull::from(&mut native_context).cast()),
        );
        let result = invoke_entry_with_function(
            &code,
            metadata.entry_offset,
            thread.as_ptr(),
            function.bits(),
            0,
            [0; 4],
            0,
        );
        ncl_sys::replace_native_context(thread, previous);
        drop(native_context);
        let (value, _) = result;
        if let Some(error) = context.thread_mut().take_native_error() {
            return Err(native_failure(error));
        }
        if let Some(error) = context.take_pending() {
            return Err(error.into());
        }
        Ok(Word::from_bits(value))
    }

    /// Format a fixnum result for the command-line frontend.
    pub fn format_result(&self, value: Word) -> String {
        value.as_fixnum().map_or_else(
            || format!("0x{:x}", value.bits()),
            |number| number.to_string(),
        )
    }

    /// Create a function caller that can invoke this runtime's published code.
    #[must_use]
    pub const fn function_caller(&self) -> RuntimeFunctionCaller {
        RuntimeFunctionCaller
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        for (_, token) in self.rooted_functions.drain(..).rev() {
            let _ = ncl_object::pop_root(&mut self.context, token);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{Runtime, RuntimeError};

    #[test]
    fn evaluates_a_literal_through_native_code() {
        let mut runtime = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => panic!("runtime initialization failed: {error}"),
        };
        let value = match runtime.eval("42") {
            Ok(value) => value,
            Err(error) => panic!("native evaluation failed: {error}"),
        };
        assert_eq!(runtime.format_result(value), "42");
        drop(runtime);
    }

    #[test]
    fn compile_and_load_share_the_native_pipeline() {
        let mut runtime = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => panic!("runtime initialization failed: {error}"),
        };
        let compiled = match runtime.compile("41") {
            Ok(value) => value,
            Err(error) => panic!("compile failed: {error}"),
        };
        let loaded = match runtime.load("42") {
            Ok(value) => value,
            Err(error) => panic!("load failed: {error}"),
        };
        assert_eq!(runtime.format_result(compiled), "41");
        assert_eq!(runtime.format_result(loaded), "42");
    }

    #[test]
    fn load_file_executes_source_and_reports_missing_files() {
        let path = std::env::temp_dir().join(format!("ncl-runtime-{}.lisp", std::process::id()));
        if let Err(error) = fs::write(&path, "43") {
            panic!("source file creation failed: {error}");
        }

        let mut runtime = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => panic!("runtime initialization failed: {error}"),
        };
        let value = match runtime.load_file(&path) {
            Ok(value) => value,
            Err(error) => panic!("load_file failed: {error}"),
        };
        let compiled = match runtime.compile_file(&path) {
            Ok(value) => value,
            Err(error) => panic!("compile_file failed: {error}"),
        };
        assert_eq!(runtime.format_result(value), "43");
        assert_eq!(runtime.format_result(compiled), "43");
        let missing = runtime.load_file(path.with_extension("missing"));
        assert!(matches!(missing, Err(RuntimeError::Io { .. })));
        if let Err(error) = fs::remove_file(path) {
            panic!("source file cleanup failed: {error}");
        }
    }

    #[test]
    fn runtimes_can_be_created_and_dropped_repeatedly() {
        for _ in 0..3 {
            let mut runtime = match Runtime::new() {
                Ok(runtime) => runtime,
                Err(error) => panic!("runtime initialization failed: {error}"),
            };
            let value = match runtime.eval("7") {
                Ok(value) => value,
                Err(error) => panic!("native evaluation failed: {error}"),
            };
            assert_eq!(runtime.format_result(value), "7");
        }
    }
}
