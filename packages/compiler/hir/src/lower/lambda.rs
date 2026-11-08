use ncl_ir::{Compare, Constant, OpKind, Param, Terminator, Ty, ValueId};

use crate::binding::{BindingKind, ScopeId};
use crate::form::{LambdaKey, LambdaOptional, LambdaSignature};
use crate::ids::{BindingId, NodeId};

use super::{FunctionLowerer, LowerError};

pub(super) fn required_signature(parameters: &[BindingId]) -> LambdaSignature {
    LambdaSignature {
        required: parameters.to_vec(),
        optional: Vec::new(),
        rest: None,
        keys: Vec::new(),
        allow_other_keys: false,
        aux: Vec::new(),
    }
}

impl FunctionLowerer<'_> {
    pub(super) fn lambda_value(
        &mut self,
        node: NodeId,
        parameters: &[BindingId],
        body_scope: ScopeId,
        body: &[NodeId],
    ) -> Result<ValueId, LowerError> {
        let signature = self
            .form
            .lambda_signature_for_lambda(node)
            .map_or_else(|| required_signature(parameters), Clone::clone);
        let definition_scope = self
            .form
            .nodes()
            .get(node)
            .ok_or(LowerError::InvalidNode(node))?
            .scope;
        let captures = capture_bindings(self.form, definition_scope, body_scope)?;
        let id = self.pool.allocate()?;
        let params = lambda_params(&signature)?;
        let closure = {
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
            bind_captures(&mut nested, &captures)?;
            bind_signature(&mut nested, &signature)?;
            let result = nested.sequence(body)?;
            if !nested.is_terminated() {
                nested.terminate(Terminator::Return {
                    values: vec![result],
                })?;
            }
            nested.finish()
        };
        self.pool.functions.push(closure);

        let entry = self.word_constant(Constant::FunctionEntry(id))?;
        let capture_values = capture_values(self, &captures)?;
        self.push_one(
            OpKind::MakeClosure {
                entry,
                captures: capture_values,
            },
            Ty::Word,
        )
    }
}

pub(super) fn capture_bindings(
    form: &crate::form::HirForm,
    definition_scope: ScopeId,
    body_scope: ScopeId,
) -> Result<Vec<CaptureBinding>, LowerError> {
    let mut captures = Vec::new();
    for binding in form.bindings().iter() {
        let outer = scope_is_ancestor(form, binding.scope(), definition_scope)?;
        if !outer {
            continue;
        }
        let mut used = false;
        for node in binding.reads().iter().chain(binding.writes()) {
            let Some(node) = form.nodes().get(*node) else {
                return Err(LowerError::Ir(
                    "verified binding use has no node".to_owned(),
                ));
            };
            if scope_is_ancestor(form, body_scope, node.scope)? {
                used = true;
                break;
            }
        }
        if used {
            let kind = match binding.kind() {
                BindingKind::LexicalVariable => CaptureKind::Variable,
                BindingKind::LocalFunction if is_recursive_function(form, binding.id()) => {
                    CaptureKind::RecursiveFunction
                }
                BindingKind::LocalFunction => CaptureKind::Function,
            };
            captures.push(CaptureBinding {
                binding: binding.id(),
                kind,
            });
        }
    }
    captures.sort_by_key(|capture| capture.binding.index());
    Ok(captures)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CaptureKind {
    Variable,
    Function,
    RecursiveFunction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CaptureBinding {
    pub(super) binding: BindingId,
    pub(super) kind: CaptureKind,
}

fn is_recursive_function(form: &crate::form::HirForm, binding: BindingId) -> bool {
    form.nodes().iter().any(|(_, node)| {
        matches!(
            &node.kind,
            crate::node::NodeKind::LocalFunctions {
                kind: crate::binding::LocalFunctionKind::Labels,
                definitions,
                ..
            } if definitions.iter().any(|definition| definition.binding == binding)
        )
    })
}

pub(super) fn scope_is_ancestor(
    form: &crate::form::HirForm,
    ancestor: ScopeId,
    mut scope: ScopeId,
) -> Result<bool, LowerError> {
    loop {
        if scope == ancestor {
            return Ok(true);
        }
        let parent = form
            .bindings()
            .scope_parent(scope)
            .map_err(|error| LowerError::Ir(error.to_string()))?;
        let Some(parent) = parent else {
            return Ok(false);
        };
        scope = parent;
    }
}

pub(super) fn bind_captures(
    lowerer: &mut FunctionLowerer<'_>,
    captures: &[CaptureBinding],
) -> Result<(), LowerError> {
    for (index, capture) in captures.iter().copied().enumerate() {
        let index = u8::try_from(index)
            .map_err(|_| LowerError::Ir("capture index exceeds u8".to_owned()))?;
        let value = lowerer.push_one(OpKind::LoadCapture { index }, Ty::Word)?;
        match capture.kind {
            CaptureKind::Variable => {
                let info = lowerer
                    .form
                    .bindings()
                    .binding(capture.binding)
                    .ok_or(LowerError::MissingLexicalValue(capture.binding))?;
                if info.writes().is_empty() {
                    let _ = lowerer.lexical_cells.remove(&capture.binding);
                } else {
                    let _ = lowerer.lexical_cells.insert(capture.binding);
                }
                let _ = lowerer.lexical_values.insert(capture.binding, value);
            }
            CaptureKind::Function => {
                let _ = lowerer.local_functions.insert(capture.binding, value);
            }
            CaptureKind::RecursiveFunction => {
                let _ = lowerer.local_function_cells.insert(capture.binding);
                let _ = lowerer.local_functions.insert(capture.binding, value);
            }
        }
    }
    Ok(())
}

pub(super) fn capture_values(
    lowerer: &FunctionLowerer<'_>,
    captures: &[CaptureBinding],
) -> Result<Vec<ValueId>, LowerError> {
    captures
        .iter()
        .map(|capture| match capture.kind {
            CaptureKind::Variable => lowerer
                .lexical_values
                .get(&capture.binding)
                .copied()
                .ok_or(LowerError::MissingLexicalValue(capture.binding)),
            CaptureKind::Function | CaptureKind::RecursiveFunction => lowerer
                .local_functions
                .get(&capture.binding)
                .copied()
                .ok_or(LowerError::MissingLocalFunction(capture.binding)),
        })
        .collect()
}

pub(super) fn lambda_params(signature: &LambdaSignature) -> Result<Vec<Param>, LowerError> {
    let fixed_arguments = signature
        .required
        .len()
        .checked_add(signature.optional.len())
        .ok_or_else(|| LowerError::Ir("lambda parameter count overflow".to_owned()))?;
    if fixed_arguments > usize::from(u8::MAX) {
        return Err(LowerError::Ir(
            "lambda parameter index exceeds u8".to_owned(),
        ));
    }
    let mut params = Vec::with_capacity(fixed_arguments + 1);
    params.push(parameter("argc"));
    params
        .extend((0..signature.required.len()).map(|index| parameter(&format!("required-{index}"))));
    params
        .extend((0..signature.optional.len()).map(|index| parameter(&format!("optional-{index}"))));
    Ok(params)
}

pub(super) fn bind_signature(
    lowerer: &mut FunctionLowerer<'_>,
    signature: &LambdaSignature,
) -> Result<(), LowerError> {
    for (index, binding) in signature.required.iter().copied().enumerate() {
        let index = argument_index(index + 1)?;
        let value = lowerer.push_one(OpKind::LoadArg { index }, Ty::Word)?;
        lowerer.bind_lexical_value(binding, value)?;
    }

    for (offset, optional) in signature.optional.iter().enumerate() {
        bind_optional(lowerer, signature.required.len(), offset, optional)?;
    }

    if signature.rest.is_some() || !signature.keys.is_empty() {
        let rest = make_rest_list(lowerer, signature.required.len() + signature.optional.len())?;
        if let Some(binding) = signature.rest {
            lowerer.bind_lexical_value(binding, rest)?;
        }
        if !signature.keys.is_empty() {
            bind_keys(lowerer, signature, rest)?;
        }
    }

    for aux in &signature.aux {
        let value = match aux.default {
            Some(node) => lowerer.node(node)?,
            None => lowerer.word_constant(Constant::Nil)?,
        };
        lowerer.bind_lexical_value(aux.binding, value)?;
    }
    Ok(())
}

fn bind_optional(
    lowerer: &mut FunctionLowerer<'_>,
    required: usize,
    offset: usize,
    optional: &LambdaOptional,
) -> Result<(), LowerError> {
    let argc_word = lowerer.push_one(OpKind::LoadArg { index: 0 }, Ty::Word)?;
    let argc = lowerer.push_one(
        OpKind::Convert {
            op: ncl_ir::Convert::WordToI64,
            value: argc_word,
        },
        Ty::I64,
    )?;
    let threshold_value = required
        .checked_add(offset)
        .and_then(|index| index.checked_add(1))
        .and_then(|index| i64::try_from(index).ok())
        .ok_or_else(|| LowerError::Ir("optional argument threshold overflow".to_owned()))?;
    let threshold = lowerer.push_constant(Constant::Fixnum(threshold_value), Ty::I64)?;
    let present = lowerer.push_one(
        OpKind::Compare {
            op: Compare::Ge,
            left: argc,
            right: threshold,
        },
        Ty::Bool,
    )?;
    let source = lowerer.current_block;
    let present_block = lowerer.create_block(Vec::new());
    let default_block = lowerer.create_block(Vec::new());
    let value_result = lowerer.builder.fresh_value();
    let supplied_result = lowerer.builder.fresh_value();
    let merge = lowerer.create_block(vec![(Ty::Word, value_result), (Ty::Word, supplied_result)]);
    lowerer.position(source)?;
    lowerer.terminate(Terminator::Branch {
        condition: present,
        then_target: present_block,
        then_args: Vec::new(),
        else_target: default_block,
        else_args: Vec::new(),
    })?;

    lowerer.position(present_block)?;
    let argument = argument_index(required + offset + 1)?;
    let value = lowerer.push_one(OpKind::LoadArg { index: argument }, Ty::Word)?;
    let supplied = lowerer.word_constant(Constant::T)?;
    lowerer.terminate(Terminator::Jump {
        target: merge,
        args: vec![value, supplied],
    })?;

    lowerer.position(default_block)?;
    let value = match optional.default {
        Some(node) => lowerer.node(node)?,
        None => lowerer.word_constant(Constant::Nil)?,
    };
    let supplied = lowerer.word_constant(Constant::Nil)?;
    lowerer.terminate(Terminator::Jump {
        target: merge,
        args: vec![value, supplied],
    })?;

    lowerer.position(merge)?;
    lowerer.bind_lexical_value(optional.binding, value_result)?;
    if let Some(binding) = optional.supplied_p {
        lowerer.bind_lexical_value(binding, supplied_result)?;
    }
    Ok(())
}

fn bind_keys(
    lowerer: &mut FunctionLowerer<'_>,
    signature: &LambdaSignature,
    rest: ValueId,
) -> Result<(), LowerError> {
    let allow_other = if signature.allow_other_keys {
        lowerer.word_constant(Constant::T)?
    } else {
        lowerer.word_constant(Constant::Nil)?
    };
    let mut allowed = lowerer.word_constant(Constant::Nil)?;
    for key in signature.keys.iter().rev() {
        let keyword = lowerer.word_constant(lowerer.symbol_constant(key.keyword)?)?;
        lowerer.push_none(OpKind::Safepoint)?;
        allowed = lowerer.push_one(
            OpKind::Builtin {
                name: "CONS".to_owned(),
                args: vec![keyword, allowed],
                specialization: None,
                specialization_disabled: false,
            },
            Ty::Word,
        )?;
    }
    lowerer.push_none(OpKind::Safepoint)?;
    let _ = lowerer.push_one(
        OpKind::Builtin {
            name: "check-keywords".to_owned(),
            args: vec![rest, allow_other, allowed],
            specialization: None,
            specialization_disabled: false,
        },
        Ty::Word,
    )?;
    for key in &signature.keys {
        bind_key(lowerer, key, rest)?;
    }
    Ok(())
}

fn bind_key(
    lowerer: &mut FunctionLowerer<'_>,
    key: &LambdaKey,
    rest: ValueId,
) -> Result<(), LowerError> {
    let keyword = lowerer.word_constant(lowerer.symbol_constant(key.keyword)?)?;
    lowerer.push_none(OpKind::Safepoint)?;
    let value = lowerer.push_one(
        OpKind::Builtin {
            name: "keyword-value".to_owned(),
            args: vec![rest, keyword],
            specialization: None,
            specialization_disabled: false,
        },
        Ty::Word,
    )?;
    lowerer.push_none(OpKind::Safepoint)?;
    let supplied = lowerer.push_one(
        OpKind::Builtin {
            name: "keyword-supplied-p".to_owned(),
            args: vec![rest, keyword],
            specialization: None,
            specialization_disabled: false,
        },
        Ty::Word,
    )?;
    let value = bind_keyword_default(lowerer, key, value, supplied)?;
    lowerer.bind_lexical_value(key.binding, value)?;
    if let Some(binding) = key.supplied_p {
        lowerer.bind_lexical_value(binding, supplied)?;
    }
    Ok(())
}

fn bind_keyword_default(
    lowerer: &mut FunctionLowerer<'_>,
    key: &LambdaKey,
    value: ValueId,
    supplied: ValueId,
) -> Result<ValueId, LowerError> {
    let nil = lowerer.word_constant(Constant::Nil)?;
    let is_present = lowerer.push_one(
        OpKind::Compare {
            op: Compare::Ne,
            left: supplied,
            right: nil,
        },
        Ty::Bool,
    )?;
    let source = lowerer.current_block;
    let present_block = lowerer.create_block(Vec::new());
    let default_block = lowerer.create_block(Vec::new());
    let result_value = lowerer.builder.fresh_value();
    let merge = lowerer.create_block(vec![(Ty::Word, result_value)]);
    lowerer.position(source)?;
    lowerer.terminate(Terminator::Branch {
        condition: is_present,
        then_target: present_block,
        then_args: Vec::new(),
        else_target: default_block,
        else_args: Vec::new(),
    })?;
    lowerer.position(present_block)?;
    lowerer.terminate(Terminator::Jump {
        target: merge,
        args: vec![value],
    })?;
    lowerer.position(default_block)?;
    let value = match key.default {
        Some(node) => lowerer.node(node)?,
        None => lowerer.word_constant(Constant::Nil)?,
    };
    lowerer.terminate(Terminator::Jump {
        target: merge,
        args: vec![value],
    })?;
    lowerer.position(merge)?;
    Ok(result_value)
}

fn make_rest_list(lowerer: &mut FunctionLowerer<'_>, start: usize) -> Result<ValueId, LowerError> {
    let start = i64::try_from(start)
        .map_err(|_| LowerError::Ir("rest parameter index overflow".to_owned()))?;
    let start = lowerer.push_constant(Constant::Fixnum(start), Ty::I64)?;
    let start = lowerer.push_one(
        OpKind::Convert {
            op: ncl_ir::Convert::I64ToWord,
            value: start,
        },
        Ty::Word,
    )?;
    let argc = lowerer.push_one(OpKind::LoadArg { index: 0 }, Ty::Word)?;
    lowerer.push_none(OpKind::Safepoint)?;
    lowerer.push_one(
        OpKind::Builtin {
            name: "make-rest-list".to_owned(),
            args: vec![argc, start],
            specialization: None,
            specialization_disabled: false,
        },
        Ty::Word,
    )
}

fn argument_index(index: usize) -> Result<u8, LowerError> {
    u8::try_from(index).map_err(|_| LowerError::Ir("argument index exceeds u8".to_owned()))
}

fn parameter(name: &str) -> Param {
    Param {
        name: name.to_owned(),
        ty: Ty::Word,
    }
}
