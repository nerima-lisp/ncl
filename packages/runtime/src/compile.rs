use std::path::Path;

use crate::{Runtime, RuntimeError};
use ncl_object::Word;
use ncl_object::{Runtime as ObjectRuntime, ThreadContext};
use ncl_reader::{ReadOptions, StringSource, read};

pub fn source(runtime: &mut Runtime, source: &str) -> Result<Word, RuntimeError> {
    runtime.eval(source)
}

pub fn file(runtime: &mut Runtime, path: &Path) -> Result<Word, RuntimeError> {
    let contents = std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
        path: path.display().to_string(),
        error,
    })?;
    source(runtime, &contents)
}

pub fn read_forms(
    context: &mut ThreadContext,
    object: &ObjectRuntime,
    source: &str,
) -> Result<Vec<Word>, RuntimeError> {
    let options = ReadOptions::standard(context, object)?;
    let mut input = StringSource::new(source);
    let mut forms = Vec::new();
    while let Some(form) = read(context, object, &mut input, &options)? {
        forms.push(form);
    }
    Ok(forms)
}
