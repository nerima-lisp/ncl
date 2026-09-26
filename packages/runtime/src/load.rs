use std::path::Path;

use crate::{Runtime, RuntimeError};
use ncl_object::Word;

pub fn file(runtime: &mut Runtime, path: &Path) -> Result<Word, RuntimeError> {
    let source = std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
        path: path.display().to_string(),
        error,
    })?;
    runtime.load(&source)
}
