use ncl_ir::{Constant, OpKind, Prim, Ty, ValueId};

use crate::ids::{BindingId, NodeId};

use super::ops::SYMBOL_FUNCTION_FIELD;
use super::{FunctionLowerer, LowerError};

impl FunctionLowerer<'_> {
    pub(super) fn produce_values(&mut self, nodes: &[NodeId]) -> Result<ValueId, LowerError> {
        let values = nodes
            .iter()
            .map(|node| self.node(*node))
            .collect::<Result<Vec<_>, _>>()?;
        self.push_none(OpKind::SetMultipleValues {
            values: values.clone(),
        })?;
        values
            .first()
            .copied()
            .map_or_else(|| self.word_constant(Constant::Nil), Ok)
    }

    pub(super) fn prog1(&mut self, first: NodeId, rest: &[NodeId]) -> Result<ValueId, LowerError> {
        let value = self.node(first)?;
        if self.is_terminated() {
            return Ok(value);
        }
        let saved = self.capture_multiple_values(value)?;
        for form in rest {
            let _ = self.node(*form)?;
            if self.is_terminated() {
                return Ok(value);
            }
        }
        self.restore_multiple_values(saved)
    }

    pub(super) fn multiple_value_bind(
        &mut self,
        value: NodeId,
        bindings: &[BindingId],
        body: &[NodeId],
    ) -> Result<ValueId, LowerError> {
        let primary = self.node(value)?;
        if self.is_terminated() {
            return Ok(primary);
        }
        let values = self.capture_multiple_values(primary)?;
        let mut allocated = Vec::with_capacity(bindings.len());
        let mut remaining = values;
        for binding in bindings {
            let value = self.push_one(
                OpKind::Prim {
                    op: Prim::Car,
                    args: vec![remaining],
                    condition: None,
                },
                Ty::Word,
            )?;
            self.bind_lexical_value(*binding, value)?;
            allocated.push(*binding);
            remaining = self.push_one(
                OpKind::Prim {
                    op: Prim::Cdr,
                    args: vec![remaining],
                    condition: None,
                },
                Ty::Word,
            )?;
        }
        let result = self.sequence(body)?;
        for binding in allocated {
            let _ = self.lexical_values.remove(&binding);
            let _ = self.lexical_cells.remove(&binding);
        }
        Ok(result)
    }

    pub(super) fn multiple_value_call(
        &mut self,
        function: NodeId,
        arguments: &[NodeId],
    ) -> Result<ValueId, LowerError> {
        let function = self.node(function)?;
        if self.is_terminated() {
            return Ok(function);
        }
        let mut value_lists = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let primary = self.node(*argument)?;
            if self.is_terminated() {
                return Ok(primary);
            }
            value_lists.push(self.capture_multiple_values(primary)?);
        }
        let mut arguments = Vec::with_capacity(value_lists.len() + 1);
        arguments.push(function);
        arguments.extend(value_lists);
        self.call_named("NCL-EXT", "MULTIPLE-VALUE-CALL-LIST", &arguments, false)
    }

    pub(super) fn capture_multiple_values(
        &mut self,
        primary: ValueId,
    ) -> Result<ValueId, LowerError> {
        self.call_named("NCL-EXT", "CAPTURE-MULTIPLE-VALUES", &[primary], false)
    }

    pub(super) fn restore_multiple_values(
        &mut self,
        values: ValueId,
    ) -> Result<ValueId, LowerError> {
        self.call_named("COMMON-LISP", "VALUES-LIST", &[values], true)
    }

    fn call_named(
        &mut self,
        package: &str,
        name: &str,
        arguments: &[ValueId],
        named_symbol: bool,
    ) -> Result<ValueId, LowerError> {
        let symbol = self.word_constant(Constant::Symbol {
            package: package.to_owned(),
            name: name.to_owned(),
        })?;
        let closure = self.load_field(symbol, SYMBOL_FUNCTION_FIELD)?;
        let argc = self.argument_count(arguments.len())?;
        let mut call_args = Vec::with_capacity(arguments.len() + 1);
        call_args.push(argc);
        call_args.extend_from_slice(arguments);
        self.push_none(OpKind::Safepoint)?;
        self.push_one(
            OpKind::CallClosure {
                closure,
                args: call_args,
                named_symbol: named_symbol.then_some(symbol),
            },
            Ty::Word,
        )
    }
}
