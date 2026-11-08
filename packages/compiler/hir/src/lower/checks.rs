use ncl_ir::{OpKind, Prim, Terminator, Ty, ValueId};

use crate::ids::{NodeId, TypeSpecConstId};
use crate::node::Literal;

use super::{FunctionLowerer, LowerError};

impl FunctionLowerer<'_> {
    pub(super) fn check_type(
        &mut self,
        node: NodeId,
        value: crate::ids::ValueId,
        expected_type: TypeSpecConstId,
    ) -> Result<ValueId, LowerError> {
        let producer = self
            .form
            .value_producer(value)
            .ok_or(LowerError::UnmappedHirValue { node, value })?;
        let block = self
            .hir_block
            .ok_or(LowerError::UnmappedHirValue { node, value })?;
        let definition_block = self.node_blocks.get(&producer).copied();
        if !self.node_available_in_block(Some(block), definition_block) {
            return Err(LowerError::NodeValueNotAvailableInBlock {
                node: producer,
                use_block: Some(block),
                definition_block,
            });
        }
        let datum = self
            .values
            .get(&producer)
            .copied()
            .ok_or(LowerError::UnmappedHirValue { node, value })?;
        let _ = self.type_assertion(node, producer, expected_type)?;
        if self.is_terminated() {
            return Ok(datum);
        }
        self.literal(node, &Literal::True)
    }

    pub(super) fn type_assertion(
        &mut self,
        node: NodeId,
        value: NodeId,
        expected_type: TypeSpecConstId,
    ) -> Result<ValueId, LowerError> {
        let datum = self.node(value)?;
        if self.is_terminated() {
            return Ok(datum);
        }
        let saved_values = self.capture_multiple_values(datum)?;
        let expected_literal = self
            .form
            .type_spec_constant(expected_type)
            .cloned()
            .ok_or(LowerError::InvalidTypeSpecConstant(expected_type))?;
        let expected = self.literal(node, &expected_literal)?;
        let condition = self.push_one(
            OpKind::Prim {
                op: Prim::Typep,
                args: vec![datum, expected],
                condition: None,
            },
            Ty::Bool,
        )?;
        let source_origin = self
            .form
            .nodes()
            .get(node)
            .ok_or(LowerError::InvalidNode(node))?
            .origin;
        let origin_index = self.push_constant(
            ncl_ir::Constant::Fixnum(i64::from(source_origin.index())),
            Ty::I64,
        )?;
        let origin = self.push_one(
            OpKind::Convert {
                op: ncl_ir::Convert::I64ToWord,
                value: origin_index,
            },
            Ty::Word,
        )?;
        let source = self.current_block;
        let success = self.create_block(Vec::new());
        let failure = self.create_block(Vec::new());
        self.position(source)?;
        self.terminate(Terminator::Branch {
            condition,
            then_target: success,
            then_args: Vec::new(),
            else_target: failure,
            else_args: Vec::new(),
        })?;

        self.position(failure)?;
        self.push_none(OpKind::Safepoint)?;
        let condition = self.push_one(
            OpKind::Builtin {
                name: "ncl_signal_type_error".to_owned(),
                args: vec![datum, expected, origin],
                specialization: None,
                specialization_disabled: false,
            },
            Ty::Word,
        )?;
        self.terminate(Terminator::Throw { condition })?;

        self.position(success)?;
        self.restore_multiple_values(saved_values)
    }
}
