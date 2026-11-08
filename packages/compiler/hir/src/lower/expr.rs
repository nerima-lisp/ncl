use ncl_ir::{Compare, Constant, OpKind, Terminator, Ty, ValueId};

use crate::ids::BindingId;
use crate::node::{
    CallTarget, CompileTimeSituation, LetKind, MultipleValueOperation, NodeKind, VariableTarget,
};

use super::ops::{SYMBOL_FUNCTION_FIELD, SYMBOL_VALUE_FIELD, VALUE_CELL_VALUE_FIELD};
use super::{FunctionLowerer, LowerError};

impl FunctionLowerer<'_> {
    pub(super) fn node(&mut self, id: crate::ids::NodeId) -> Result<ValueId, LowerError> {
        if let Some(value) = self.values.get(&id).copied() {
            let use_block = self.hir_block;
            let definition_block = self.node_blocks.get(&id).copied();
            if !self.node_available_in_block(use_block, definition_block) {
                return Err(LowerError::NodeValueNotAvailableInBlock {
                    node: id,
                    use_block,
                    definition_block,
                });
            }
            return Ok(value);
        }
        if !self.lowering_nodes.insert(id) {
            return Err(LowerError::CyclicNode(id));
        }
        let lowered = self.lower_node(id);
        let _ = self.lowering_nodes.remove(&id);
        let value = lowered?;
        let _ = self.values.insert(id, value);
        if let Some(block) = self.hir_block {
            let _ = self.node_blocks.insert(id, block);
        }
        Ok(value)
    }

    fn lower_node(&mut self, id: crate::ids::NodeId) -> Result<ValueId, LowerError> {
        let node = self
            .form
            .nodes()
            .get(id)
            .ok_or(LowerError::InvalidNode(id))?;
        let kind = node.kind.clone();
        let value = match kind {
            NodeKind::Literal(literal) => self.literal(id, &literal)?,
            NodeKind::VariableReference { target } => self.variable(id, target)?,
            NodeKind::Assignment { target, value } => self.assignment(id, target, value)?,
            NodeKind::Call { target, arguments } => self.call(id, target, &arguments)?,
            NodeKind::If {
                test,
                then_branch,
                else_branch,
            } => self.conditional(test, then_branch, else_branch)?,
            NodeKind::Sequence { forms, .. } => self.sequence(&forms)?,
            NodeKind::Block { body, .. } => self.lower_block(id, &body)?,
            NodeKind::Let {
                kind,
                bindings,
                body,
                ..
            } => self.let_form(kind, &bindings, &body)?,
            NodeKind::MultipleValues(operation) => self.multiple_values(operation)?,
            NodeKind::CompileTimeForm { situations, form } => {
                self.compile_time_form(&situations, form)?
            }
            NodeKind::TypeAssertion { value, .. } if self.node_policy(id).safety().value() == 0 => {
                self.node(value)?
            }
            NodeKind::FunctionReference { target } => self.function_reference(id, target)?,
            NodeKind::ClosureReference { function } => self.node(function)?,
            NodeKind::LoadTimeValue { form, read_only } => {
                self.load_time_value(id, form, read_only)?
            }
            NodeKind::Lambda {
                parameters,
                body_scope,
                body,
            } => self.lambda_value(id, &parameters, body_scope, &body)?,
            NodeKind::LocalFunctions {
                kind,
                definitions,
                body,
                ..
            } => self.local_functions(id, kind, &definitions, &body)?,
            NodeKind::DynamicBinding {
                symbols,
                values,
                body,
            } => self.dynamic_binding(id, symbols, values, &body)?,
            NodeKind::ReturnFrom { target, value } => self.return_from(id, target, value)?,
            NodeKind::Tagbody { tags, .. } => self.tagbody(id, &tags)?,
            NodeKind::Go { target } => self.go(id, target)?,
            NodeKind::Catch { tag, body } => self.catch(id, tag, &body)?,
            NodeKind::Throw { tag, value } => self.throw(id, tag, value)?,
            NodeKind::UnwindProtect {
                cleanup,
                protected,
                cleanup_forms,
            } => self.unwind_protect(id, cleanup, protected, &cleanup_forms)?,
            NodeKind::NonLocalExit {
                target,
                values,
                cleanups,
            } => self.non_local_exit(id, target, &values, &cleanups)?,
            NodeKind::TypeAssertion {
                value,
                expected_type,
                ..
            } => self.type_assertion(id, value, expected_type)?,
            NodeKind::KnownFunctionOperation { .. } => {
                return Self::unsupported(id, "known-function-operation");
            }
            NodeKind::SequenceOperation { .. } => {
                return Self::unsupported(id, "sequence-operation");
            }
            NodeKind::ArrayOperation { .. } => return Self::unsupported(id, "array-operation"),
            NodeKind::RuntimeCheck { .. } => return Self::unsupported(id, "runtime-check"),
            NodeKind::CheckType {
                value,
                expected_type,
                ..
            } => self.check_type(id, value, expected_type)?,
            NodeKind::TopLevelDefinition { .. } => {
                return Self::unsupported(id, "top-level-definition");
            }
        };
        Ok(value)
    }

    fn multiple_values(
        &mut self,
        operation: MultipleValueOperation,
    ) -> Result<ValueId, LowerError> {
        match operation {
            MultipleValueOperation::Produce { values }
            | MultipleValueOperation::Return { values } => self.produce_values(&values),
            MultipleValueOperation::Prog1 { first, rest } => self.prog1(first, &rest),
            MultipleValueOperation::Bind {
                value,
                bindings,
                body,
                ..
            } => self.multiple_value_bind(value, &bindings, &body),
            MultipleValueOperation::Call {
                function,
                arguments,
            } => self.multiple_value_call(function, &arguments),
        }
    }

    fn compile_time_form(
        &mut self,
        situations: &[CompileTimeSituation],
        form: crate::ids::NodeId,
    ) -> Result<ValueId, LowerError> {
        if situations.iter().any(|situation| {
            matches!(
                situation,
                CompileTimeSituation::Execute | CompileTimeSituation::Eval
            )
        }) {
            self.node(form)
        } else {
            self.word_constant(Constant::Nil)
        }
    }

    fn load_time_value(
        &mut self,
        node: crate::ids::NodeId,
        form: crate::ids::NodeId,
        read_only: bool,
    ) -> Result<ValueId, LowerError> {
        let Some(form_node) = self.form.nodes().get(form) else {
            return Err(LowerError::InvalidNode(form));
        };
        match &form_node.kind {
            NodeKind::Literal(
                crate::node::Literal::Nil
                | crate::node::Literal::True
                | crate::node::Literal::Symbol(_)
                | crate::node::Literal::Number(_)
                | crate::node::Literal::Character(_),
            ) => self.node(form),
            NodeKind::Literal(_) if read_only => self.node(form),
            _ => Self::unsupported(node, "load-time-value-with-runtime-form"), // check-added-lines: allow(wildcard) runtime load-time forms must fail closed
        }
    }

    fn variable(
        &mut self,
        _node: crate::ids::NodeId,
        target: VariableTarget,
    ) -> Result<ValueId, LowerError> {
        match target {
            VariableTarget::Lexical(binding) => {
                let value = self
                    .lexical_values
                    .get(&binding)
                    .copied()
                    .ok_or(LowerError::MissingLexicalValue(binding))?;
                if self.lexical_cells.contains(&binding) {
                    self.load_field(value, VALUE_CELL_VALUE_FIELD)
                } else {
                    Ok(value)
                }
            }
            VariableTarget::Global(symbol) | VariableTarget::Special(symbol) => {
                let symbol = self.word_constant(self.symbol_constant(symbol)?)?;
                self.bound_symbol_value(symbol)
            }
        }
    }

    fn assignment(
        &mut self,
        node: crate::ids::NodeId,
        target: VariableTarget,
        value: crate::ids::NodeId,
    ) -> Result<ValueId, LowerError> {
        let value = self.node(value)?;
        match target {
            VariableTarget::Lexical(binding) => {
                let Some(location) = self.lexical_values.get(&binding).copied() else {
                    return Err(LowerError::MissingLexicalValue(binding));
                };
                if self.lexical_cells.contains(&binding) {
                    self.push_none(OpKind::StoreField {
                        object: location,
                        field: VALUE_CELL_VALUE_FIELD,
                        value,
                    })?;
                } else {
                    if self.branch_depth != 0 {
                        return Self::unsupported(node, "uncelled-assignment-in-conditional");
                    }
                    let _ = self.lexical_values.insert(binding, value);
                }
                Ok(value)
            }
            VariableTarget::Global(symbol) | VariableTarget::Special(symbol) => {
                let symbol = self.word_constant(self.symbol_constant(symbol)?)?;
                self.push_none(OpKind::StoreField {
                    object: symbol,
                    field: SYMBOL_VALUE_FIELD,
                    value,
                })?;
                Ok(value)
            }
        }
    }

    fn let_form(
        &mut self,
        kind: LetKind,
        bindings: &[crate::node::LetBinding],
        body: &[crate::ids::NodeId],
    ) -> Result<ValueId, LowerError> {
        let mut allocated = Vec::with_capacity(bindings.len());
        if kind == LetKind::Parallel {
            for binding in bindings {
                let value = self.initializer(binding.initializer)?;
                if self.is_terminated() {
                    break;
                }
                allocated.push((binding.binding, value));
            }
            if !self.is_terminated() {
                for (binding, value) in &allocated {
                    self.bind_lexical_value(*binding, *value)?;
                }
            }
        } else {
            for binding in bindings {
                let value = self.initializer(binding.initializer)?;
                if self.is_terminated() {
                    break;
                }
                self.bind_lexical_value(binding.binding, value)?;
                allocated.push((binding.binding, value));
            }
        }
        let result = if self.is_terminated() {
            self.word_constant(Constant::Nil)?
        } else {
            self.sequence(body)?
        };
        for (binding, _) in allocated {
            let _ = self.lexical_values.remove(&binding);
            let _ = self.lexical_cells.remove(&binding);
        }
        Ok(result)
    }

    fn initializer(
        &mut self,
        initializer: Option<crate::ids::NodeId>,
    ) -> Result<ValueId, LowerError> {
        match initializer {
            Some(node) => self.node(node),
            None => self.word_constant(Constant::Nil),
        }
    }

    pub(super) fn sequence(&mut self, nodes: &[crate::ids::NodeId]) -> Result<ValueId, LowerError> {
        if nodes.is_empty() {
            return self.word_constant(Constant::Nil);
        }
        let mut result = self.word_constant(Constant::Nil)?;
        for node in nodes {
            if self.is_terminated() {
                break;
            }
            result = self.node(*node)?;
        }
        Ok(result)
    }

    fn node_policy(&self, id: crate::ids::NodeId) -> crate::policy::OptimizationPolicy {
        self.form
            .nodes()
            .get(id)
            .and_then(|node| self.form.scope_metadata().get(&node.scope))
            .map_or_else(|| self.form.policy(), |metadata| metadata.policy)
    }

    pub(super) fn bind_lexical_value(
        &mut self,
        binding: BindingId,
        value: ValueId,
    ) -> Result<(), LowerError> {
        let info = self
            .form
            .bindings()
            .binding(binding)
            .ok_or(LowerError::MissingLexicalValue(binding))?;
        if info.writes().is_empty() {
            let _ = self.lexical_cells.remove(&binding);
            let _ = self.lexical_values.insert(binding, value);
        } else {
            let cell = self.push_one(OpKind::MakeValueCell { value }, Ty::Word)?;
            let _ = self.lexical_cells.insert(binding);
            let _ = self.lexical_values.insert(binding, cell);
        }
        Ok(())
    }

    fn call(
        &mut self,
        _node: crate::ids::NodeId,
        target: CallTarget,
        arguments: &[crate::ids::NodeId],
    ) -> Result<ValueId, LowerError> {
        let (closure, named_symbol) = match target {
            CallTarget::Global(symbol) | CallTarget::Intrinsic(symbol) => {
                let symbol_value = self.word_constant(self.symbol_constant(symbol)?)?;
                let function = self.load_field(symbol_value, SYMBOL_FUNCTION_FIELD)?;
                (function, Some(symbol_value))
            }
            CallTarget::Value(function) => (self.node(function)?, None),
            CallTarget::Local(binding) => (self.local_function_value(binding)?, None),
        };
        let values = arguments
            .iter()
            .map(|argument| self.node(*argument))
            .collect::<Result<Vec<_>, _>>()?;
        let argc = self.argument_count(values.len())?;
        let mut call_args = Vec::with_capacity(values.len() + 1);
        call_args.push(argc);
        call_args.extend(values);
        self.push_none(OpKind::Safepoint)?;
        self.push_one(
            OpKind::CallClosure {
                closure,
                args: call_args,
                named_symbol,
            },
            Ty::Word,
        )
    }

    fn function_reference(
        &mut self,
        _node: crate::ids::NodeId,
        target: CallTarget,
    ) -> Result<ValueId, LowerError> {
        match target {
            CallTarget::Global(symbol) | CallTarget::Intrinsic(symbol) => {
                let symbol = self.word_constant(self.symbol_constant(symbol)?)?;
                self.load_field(symbol, SYMBOL_FUNCTION_FIELD)
            }
            CallTarget::Value(value) => self.node(value),
            CallTarget::Local(binding) => self.local_function_value(binding),
        }
    }

    fn local_function_value(&mut self, binding: BindingId) -> Result<ValueId, LowerError> {
        let value = self
            .local_functions
            .get(&binding)
            .copied()
            .ok_or(LowerError::MissingLocalFunction(binding))?;
        if self.local_function_cells.contains(&binding) {
            self.load_field(value, VALUE_CELL_VALUE_FIELD)
        } else {
            Ok(value)
        }
    }

    fn bound_symbol_value(&mut self, symbol: ValueId) -> Result<ValueId, LowerError> {
        let value = self.load_field(symbol, SYMBOL_VALUE_FIELD)?;
        let unbound = self.word_constant(Constant::Unbound)?;
        let is_unbound = self.push_one(
            OpKind::Compare {
                op: Compare::Eq,
                left: value,
                right: unbound,
            },
            Ty::Bool,
        )?;
        let start = self.current_block;
        let slow = self.create_block(Vec::new());
        let fast = self.create_block(Vec::new());
        let result = self.builder.fresh_value();
        let merge = self.create_block(vec![(Ty::Word, result)]);
        self.position(start)?;
        self.terminate(Terminator::Branch {
            condition: is_unbound,
            then_target: slow,
            then_args: Vec::new(),
            else_target: fast,
            else_args: Vec::new(),
        })?;

        self.position(fast)?;
        self.terminate(Terminator::Jump {
            target: merge,
            args: vec![value],
        })?;

        self.position(slow)?;
        let helper = self.word_constant(Constant::Symbol {
            package: "NCL-EXT".to_owned(),
            name: "BOUND-SYMBOL-VALUE".to_owned(),
        })?;
        let function = self.load_field(helper, SYMBOL_FUNCTION_FIELD)?;
        let argc = self.argument_count(1)?;
        self.push_none(OpKind::Safepoint)?;
        let signaled = self.push_one(
            OpKind::CallClosure {
                closure: function,
                args: vec![argc, symbol],
                named_symbol: Some(helper),
            },
            Ty::Word,
        )?;
        self.terminate(Terminator::Jump {
            target: merge,
            args: vec![signaled],
        })?;
        self.position(merge)?;
        Ok(result)
    }
}
