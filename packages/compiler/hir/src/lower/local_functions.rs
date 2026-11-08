use ncl_ir::{Constant, OpKind, Terminator, Ty};

use crate::binding::LocalFunctionKind;
use crate::form::LambdaSignature;
use crate::ids::NodeId;
use crate::node::LocalFunctionDefinition;

use super::lambda::{
    CaptureBinding, bind_captures, capture_bindings, capture_values, lambda_params,
    required_signature,
};
use super::ops::VALUE_CELL_VALUE_FIELD;
use super::{FunctionLowerer, LowerError};

impl FunctionLowerer<'_> {
    pub(super) fn local_functions(
        &mut self,
        node: NodeId,
        kind: LocalFunctionKind,
        definitions: &[LocalFunctionDefinition],
        body: &[NodeId],
    ) -> Result<ncl_ir::ValueId, LowerError> {
        let captures = definitions
            .iter()
            .map(|definition| {
                capture_bindings(
                    self.form,
                    definition.definition_scope,
                    definition.body_scope,
                )
                .map(|captures| (definition.binding, definition, captures))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if kind == LocalFunctionKind::Labels {
            for (binding, _, _) in &captures {
                let nil = self.word_constant(Constant::Nil)?;
                let cell = self.push_one(OpKind::MakeValueCell { value: nil }, Ty::Word)?;
                let _ = self.local_functions.insert(*binding, cell);
                let _ = self.local_function_cells.insert(*binding);
            }
        }

        let mut installed = Vec::with_capacity(definitions.len());
        for (binding, definition, captures) in captures {
            let signature = self
                .form
                .lambda_signature_for_local_function(binding)
                .map_or_else(|| required_signature(&definition.parameters), Clone::clone);
            let function = self.lower_local_function(node, definition, &signature, &captures)?;
            if kind == LocalFunctionKind::Labels {
                let cell = self
                    .local_functions
                    .get(&binding)
                    .copied()
                    .ok_or(LowerError::MissingLocalFunction(binding))?;
                self.push_none(OpKind::StoreField {
                    object: cell,
                    field: VALUE_CELL_VALUE_FIELD,
                    value: function,
                })?;
            } else {
                let _ = self.local_functions.insert(binding, function);
            }
            installed.push(binding);
        }

        let result = self.sequence(body)?;
        for binding in installed {
            let _ = self.local_functions.remove(&binding);
            let _ = self.local_function_cells.remove(&binding);
        }
        Ok(result)
    }

    fn lower_local_function(
        &mut self,
        _node: NodeId,
        definition: &LocalFunctionDefinition,
        signature: &LambdaSignature,
        captures: &[CaptureBinding],
    ) -> Result<ncl_ir::ValueId, LowerError> {
        let id = self.pool.allocate()?;
        let params = lambda_params(signature)?;
        let function = {
            let form = self.form;
            let symbols = self.symbols;
            let pool = &mut *self.pool;
            let mut nested = FunctionLowerer::for_function(
                form,
                symbols,
                pool,
                id,
                format!("lambda-{id:?}"),
                params,
            );
            bind_captures(&mut nested, captures)?;
            super::lambda::bind_signature(&mut nested, signature)?;
            let result = nested.sequence(&definition.body)?;
            if !nested.is_terminated() {
                nested.terminate(Terminator::Return {
                    values: vec![result],
                })?;
            }
            nested.finish()
        };
        self.pool.functions.push(function);

        let entry = self.word_constant(Constant::FunctionEntry(id))?;
        let captures = capture_values(self, captures)?;
        self.push_one(OpKind::MakeClosure { entry, captures }, Ty::Word)
    }
}
