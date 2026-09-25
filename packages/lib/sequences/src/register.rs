use crate::{builtins, domain};
use ncl_object::{ObjectError, Runtime, ThreadContext};

/// Register every Phase 1 function owned by the sequences crate.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for line in include_str!("../ownership.tsv").lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() < 3 || fields[0] != "COMMON-LISP" || !fields[2].contains("function") {
            continue;
        }
        let name = fields[1];
        let implementation = domain::set_entry(name)
            .or_else(|| domain::map_entry(name))
            .or_else(|| builtins::entry(name))
            .unwrap_or_else(builtins::unsupported_implementation);
        runtime.register_builtin(&mut ctx, builtins::identifier(name), implementation)?;
    }
    Ok(())
}
