use std::collections::HashMap;

use ncl_object::{
    HeapRootedWord, ObjectError, Runtime, ThreadContext, Word, make_string, make_symbol,
};

pub fn resolve(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    identity: u32,
    name: &str,
    symbols: &mut HashMap<u32, HeapRootedWord>,
) -> Result<Word, ObjectError> {
    if let Some(symbol) = symbols.get(&identity) {
        return Ok(symbol.get());
    }
    let name = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
    let symbol = make_symbol(ctx, runtime, name)?;
    symbols.insert(identity, HeapRootedWord::new(ctx, symbol)?);
    Ok(symbol)
}
