use crate::load::keyword_name;
use crate::{Runtime, RuntimeError};
use ncl_compiler_front::{FrontError, SymbolRef};
use ncl_object::{ObjectRef, Word, classify_object};

#[derive(Clone, Copy)]
pub enum TopLevelMode {
    Execute,
    CompileFile,
}

pub fn eval_when_start(
    runtime: &mut Runtime,
    arguments: &[Word],
    mode: TopLevelMode,
) -> Result<(usize, bool), RuntimeError> {
    let Some(first) = arguments.first() else {
        return Err(malformed("missing situations"));
    };
    let situation_words = if *first == Word::NIL {
        Vec::new()
    } else if matches!(
        classify_object(&runtime.context, *first),
        ObjectRef::Cons(_)
    ) {
        ncl_compiler_front::form::list(&mut runtime.context, *first)?
    } else {
        vec![*first]
    };
    let mut active = false;
    for situation in situation_words {
        let Some(name) = keyword_name(&runtime.context, &runtime.object, situation)? else {
            return Err(malformed("eval-when situations must be keywords"));
        };
        let known = name == "COMPILE-TOPLEVEL"
            || name == "LOAD-TOPLEVEL"
            || name == "EXECUTE"
            || name == "EVAL";
        if !known {
            return Err(malformed(&format!("unknown eval-when situation :{name}")));
        }
        active |= if name == "COMPILE-TOPLEVEL" {
            matches!(mode, TopLevelMode::CompileFile)
        } else {
            matches!(mode, TopLevelMode::Execute)
        };
    }
    Ok((1, active))
}

fn malformed(detail: &str) -> RuntimeError {
    RuntimeError::Front(FrontError::MalformedForm {
        operator: SymbolRef::interned("COMMON-LISP", "EVAL-WHEN"),
        detail: detail.to_owned(),
    })
}
