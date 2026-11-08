use std::collections::{BTreeMap, HashMap};

use ncl_object::{HeapRootedWord, Word};

use super::NativeCompilation;
use crate::{PublishedFunction, Runtime, RuntimeError};

impl Runtime {
    pub(crate) fn make_function_object(
        &mut self,
        function: &ncl_ir::Function,
        compiled: &NativeCompilation,
        pending_functions: &BTreeMap<u32, PublishedFunction>,
        uninterned_symbols: &mut HashMap<u32, HeapRootedWord>,
        literal_objects: &mut HashMap<u32, HeapRootedWord>,
    ) -> Result<(HeapRootedWord, HeapRootedWord), RuntimeError> {
        let entry = compiled.entry_address()?;
        let constants = self.make_constants(
            function,
            pending_functions,
            uninterned_symbols,
            literal_objects,
        )?;
        let code_object = ncl_object::make_code_object(
            &mut self.context,
            &self.object,
            entry,
            compiled.size,
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
        Ok((
            HeapRootedWord::new(&self.context, function_object.as_word())?,
            HeapRootedWord::new(&self.context, code_object.as_word())?,
        ))
    }

    pub(crate) fn make_constants(
        &mut self,
        function: &ncl_ir::Function,
        pending_functions: &BTreeMap<u32, PublishedFunction>,
        uninterned_symbols: &mut HashMap<u32, HeapRootedWord>,
        literal_objects: &mut HashMap<u32, HeapRootedWord>,
    ) -> Result<Word, RuntimeError> {
        let mut values = Vec::with_capacity(function.constants.len());
        let mut roots = Vec::with_capacity(function.constants.len());
        for constant in &function.constants {
            let value = if let ncl_ir::Constant::FunctionEntry(id) = constant {
                let entry = pending_functions
                    .get(&id.0)
                    .map(|function| function.entry)
                    .or_else(|| {
                        self.evaluator
                            .state
                            .functions
                            .read()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .get(&id.0)
                            .map(|function| function.entry)
                    })
                    .ok_or_else(|| {
                        RuntimeError::Native(format!("function entry {} is unavailable", id.0))
                    })?;
                Word::from_bits(u64::try_from(entry).map_err(|_| {
                    RuntimeError::Native("function entry does not fit a machine word".to_owned())
                })?)
            } else {
                crate::constants::resolve_constant(
                    &mut self.context,
                    &self.object,
                    constant,
                    &values,
                    uninterned_symbols,
                    literal_objects,
                )?
            };
            values.push(value);
            let value = values
                .last_mut()
                .ok_or_else(|| RuntimeError::Native("constant table append failed".to_owned()))?;
            roots.push(ncl_object::push_root(&mut self.context, value));
        }
        let result = ncl_object::make_simple_vector(&mut self.context, &self.object, &values)
            .map_err(Into::into);
        for token in roots.into_iter().rev() {
            let _ = ncl_object::pop_root(&mut self.context, token);
        }
        result
    }
}
