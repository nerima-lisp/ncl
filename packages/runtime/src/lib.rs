//! Evaluation, compilation, loading, and registration orchestration.

mod compile;
mod load;

use ncl_codegen::{RuntimeAbi, RuntimeFunction};
use ncl_compiler_front::{FormExpander, MacroCaller, MacroRegistry, lower_toplevel};
use ncl_object::{
    FunctionObject, ObjectError, Package, Runtime as ObjectRuntime, ThreadContext, Word,
    symbol_function,
};
use ncl_sys::{
    CodeObjectMetadata, CodePtr, SafepointMap, SourceLocation, alloc_code, invoke_entry,
    publish_code, register_code, thread_layout, write_code,
};

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
    object: ObjectRuntime,
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
        if !lowered.nested.is_empty() {
            return Err(RuntimeError::Native(
                "nested functions are not yet publishable".to_owned(),
            ));
        }
        let mut module = ncl_opt::Module {
            functions: vec![lowered.entry],
        };
        let mut passes = ncl_opt::PassManager::new();
        passes.add_function_pass(ncl_opt::InlineDirectCalls::default());
        passes
            .run(&mut module)
            .map_err(|error| RuntimeError::Native(error.to_string()))?;
        let entry = module.functions.pop().ok_or_else(|| {
            RuntimeError::Native("optimization removed entry function".to_owned())
        })?;
        let abi = NativeAbi;
        let compiled = if cfg!(target_arch = "aarch64") {
            ncl_codegen::compile_function_aarch64(&entry, &abi)
        } else {
            ncl_codegen::compile_function_x86_64(&entry, &abi)
        }
        .map_err(|error| RuntimeError::Native(error.to_string()))?;
        let mut code = alloc_code(compiled.code.len())
            .map_err(|error| RuntimeError::Native(format!("{error:?}")))?;
        write_code(&mut code, 0, &compiled.code)
            .map_err(|error| RuntimeError::Native(format!("{error:?}")))?;
        publish_code(&mut code).map_err(|error| RuntimeError::Native(format!("{error:?}")))?;
        let maps = encode_maps(&compiled.safepoint_maps)?;
        let safepoint_map = SafepointMap::decode(&maps, compiled.safepoint_maps.len())
            .map_err(|error| RuntimeError::Native(error.to_owned()))?;
        let metadata = CodeObjectMetadata {
            entry_offset: compiled.entry_offset as usize,
            size: compiled.code.len(),
            frame_words: u16::try_from(compiled.frame_size / 8)
                .map_err(|_| RuntimeError::Native("frame is too large".to_owned()))?,
            function_name: "toplevel".to_owned(),
            source_locations: Vec::<SourceLocation>::new(),
            constant_slots: Vec::new(),
            safepoint_map,
            debug_table: Vec::new(),
        };
        if !compiled.safepoint_maps.is_empty() {
            register_code(self.context.thread_mut(), &code, metadata)
                .map_err(|error| RuntimeError::Native(format!("{error:?}")))?;
        }
        let (value, _) = invoke_entry(
            &code,
            compiled.entry_offset as usize,
            self.context.thread_mut(),
            0,
            [0; 4],
            0,
        );
        self.code.push(code);
        Ok(Word::from_bits(value))
    }

    /// Format a fixnum result for the command-line frontend.
    pub fn format_result(&self, value: Word) -> String {
        value.as_fixnum().map_or_else(
            || format!("0x{:x}", value.bits()),
            |number| number.to_string(),
        )
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

fn encode_maps(maps: &[ncl_codegen::SafepointMap]) -> Result<Vec<u8>, RuntimeError> {
    maps.iter().try_fold(Vec::new(), |mut bytes, map| {
        bytes.extend(
            map.encode()
                .map_err(|error| RuntimeError::Native(error.to_string()))?,
        );
        Ok(bytes)
    })
}

struct RuntimeMacroCaller;
impl MacroCaller for RuntimeMacroCaller {
    fn call_macro(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &ObjectRuntime,
        name: &ncl_compiler_front::SymbolRef,
        form: Word,
    ) -> Result<Word, ncl_compiler_front::FrontError> {
        let package =
            name.package_name()
                .ok_or_else(|| ncl_compiler_front::FrontError::MacroExpansion {
                    name: name.clone(),
                    detail: "uninterned macro has no function cell".to_owned(),
                })?;
        let package_word = runtime.find_package(ctx, package).ok_or_else(|| {
            ncl_compiler_front::FrontError::MacroExpansion {
                name: name.clone(),
                detail: "macro package is not present".to_owned(),
            }
        })?;
        let (symbol, _) = Package::from_word(package_word)
            .intern(ctx, runtime, &name.name)
            .map_err(|error| ncl_compiler_front::FrontError::MacroExpansion {
                name: name.clone(),
                detail: error.to_string(),
            })?;
        let function = symbol_function(ctx, symbol).map_err(|error| {
            ncl_compiler_front::FrontError::MacroExpansion {
                name: name.clone(),
                detail: error.to_string(),
            }
        })?;
        let function = FunctionObject::try_from(function).map_err(|error| {
            ncl_compiler_front::FrontError::MacroExpansion {
                name: name.clone(),
                detail: error.to_string(),
            }
        })?;
        runtime
            .call_builtin(ctx, function, &[form])
            .map_err(|error| ncl_compiler_front::FrontError::MacroExpansion {
                name: name.clone(),
                detail: error.to_string(),
            })
    }
}

#[derive(Clone, Copy, Debug)]
struct NativeAbi;
impl RuntimeAbi for NativeAbi {
    fn builtin_address(&self, _name: &str) -> Option<u64> {
        None
    }
    fn context_offset(&self, _field: &str) -> Option<i32> {
        None
    }
    fn field_offset(&self, field: ncl_codegen::ContextField) -> Option<i32> {
        let layout = thread_layout();
        let offset = match field {
            ncl_codegen::ContextField::TlabBump => layout.tlab_bump,
            ncl_codegen::ContextField::TlabLimit => layout.tlab_limit,
            ncl_codegen::ContextField::SafepointRequest => layout.safepoint_request,
            _ => return None,
        };
        i32::try_from(offset).ok()
    }
    fn runtime_address(&self, _function: RuntimeFunction, _name: Option<&str>) -> Option<u64> {
        None
    }
}
