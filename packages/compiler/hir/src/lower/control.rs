use ncl_ir::{Compare, Constant, OpKind, Terminator, Ty, ValueId};

use crate::ids::NodeId;

use super::{FunctionLowerer, LowerError};

impl FunctionLowerer<'_> {
    pub(super) fn conditional(
        &mut self,
        test: NodeId,
        then_branch: NodeId,
        else_branch: Option<NodeId>,
    ) -> Result<ValueId, LowerError> {
        let test = self.node(test)?;
        if self.is_terminated() {
            return Ok(test);
        }
        let nil = self.word_constant(Constant::Nil)?;
        let condition = self.push_one(
            OpKind::Compare {
                op: Compare::Ne,
                left: test,
                right: nil,
            },
            Ty::Bool,
        )?;
        let branch = self.current_block;
        let then_block = self.create_block(Vec::new());
        let else_block = self.create_block(Vec::new());
        let result = self.builder.fresh_value();
        let merge = self.create_block(vec![(Ty::Word, result)]);
        self.position(branch)?;
        self.terminate(Terminator::Branch {
            condition,
            then_target: then_block,
            then_args: Vec::new(),
            else_target: else_block,
            else_args: Vec::new(),
        })?;

        let outer_handlers = self.active_handlers.clone();
        self.position(then_block)?;
        self.active_handlers.clone_from(&outer_handlers);
        self.branch_depth += 1;
        let then_value = self.node(then_branch);
        self.branch_depth -= 1;
        let then_value = then_value?;
        let then_falls_through = !self.is_terminated();
        if then_falls_through {
            self.terminate(Terminator::Jump {
                target: merge,
                args: vec![then_value],
            })?;
        }

        self.position(else_block)?;
        self.active_handlers.clone_from(&outer_handlers);
        self.branch_depth += 1;
        let else_value = match else_branch {
            Some(branch) => self.node(branch),
            None => self.word_constant(Constant::Nil),
        };
        self.branch_depth -= 1;
        let else_value = else_value?;
        let else_falls_through = !self.is_terminated();
        if else_falls_through {
            self.terminate(Terminator::Jump {
                target: merge,
                args: vec![else_value],
            })?;
        }
        self.position(merge)?;
        self.active_handlers = outer_handlers;
        if !then_falls_through && !else_falls_through {
            let nil = self.word_constant(Constant::Nil)?;
            self.terminate(Terminator::Return { values: vec![nil] })?;
            return Ok(nil);
        }
        Ok(result)
    }

    pub(super) fn create_block(&mut self, params: Vec<(Ty, ValueId)>) -> ncl_ir::BlockId {
        let id = self.builder.create_block(params);
        self.created_blocks.push(id);
        self.current_block = id;
        id
    }

    pub(super) fn position(&mut self, block: ncl_ir::BlockId) -> Result<(), LowerError> {
        self.builder.position_at(block).map_err(LowerError::Ir)?;
        self.current_block = block;
        Ok(())
    }

    pub(super) fn terminate(&mut self, terminator: Terminator) -> Result<(), LowerError> {
        self.builder
            .terminate(terminator.clone())
            .map_err(LowerError::Ir)?;
        let _ = self.terminators.insert(self.current_block, terminator);
        let _ = self.terminated_blocks.insert(self.current_block);
        Ok(())
    }

    pub(super) fn is_terminated(&self) -> bool {
        self.terminated_blocks.contains(&self.current_block)
    }
}
