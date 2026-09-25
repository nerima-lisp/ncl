//! Lowering the frozen AST into `ncl-ir`.
//!
//! [`lower_toplevel`] turns one expanded form into an entry `ncl_ir::Function`
//! plus one `ncl_ir::Function` per lambda the form contains. Every generated
//! function satisfies [`ncl_ir::verify`]; the lowering tests assert this.
//!
//! # Calling convention of the generated IR
//!
//! A generated function takes a leading `argc` parameter of type `Word`
//! followed by its captured variables and its declared parameters, and returns
//! one `Word`. `argc` is the number of actual arguments the caller supplied,
//! excluding the captures and the `argc` slot itself, so an `&optional`
//! parameter can compare it against its position. The entry function produced by
//! [`lower_toplevel`] takes no parameters.
//!
//! # Lowering rules
//!
//! These implement "front end の lowering 規則" of `compiler-pipeline.md`:
//!
//! - A lexical variable that a lambda captures and that some form assigns is
//!   boxed in a one-word cell (`Alloc { words: 1 }`) the closures capture by
//!   reference; a read-only capture is passed by value.
//! - A `block` or `tagbody` whose `return-from`/`go` stay inside one function is
//!   lowered to `Jump` between basic blocks.
//! - `multiple-value-prog1` preserves its first form's value across the rest and
//!   records it with `SetMultipleValues`; `multiple-value-call` goes through the
//!   variadic adapter builtin.
//! - `&optional` parameters lower to `LoadArg`, an `argc` comparison, and a
//!   default block.
//!
//! # IR v2 lowering
//!
//! The v2 path below uses `FunctionEntry` constants and closure operations for
//! lambdas, and records dynamic control constructs with `HandlerRegion`s. The
//! runtime still owns the meaning of the named builtins and the actual unwind
//! transfer; the front end only emits their frozen IR contract.

mod capture;
mod env;
mod error;
mod function;
mod lambda;
mod literal;

pub use error::LowerError;

use ncl_ir::Function;

use crate::ast::Expr;

/// The functions produced by lowering one form.
#[derive(Clone, Debug)]
pub struct Lowered {
    /// The function for the lowered form itself.
    pub entry: Function,
    /// One function per lambda expression in the form, in lowering order.
    pub nested: Vec<Function>,
}

/// Lower one expanded form into an entry function and its nested functions.
///
/// The entry function takes no parameters and returns one `Word`. The nested
/// functions are the lambdas the form contains; `ncl-ir` cannot yet name a
/// function entry, so they are returned alongside the entry for the runtime to
/// register and link.
///
/// # Errors
///
/// Returns [`LowerError`] when a form has no `ncl-ir` representation.
pub fn lower_toplevel(expr: &Expr) -> Result<Lowered, LowerError> {
    let mut module = function::Lowerer::new();
    let mut entry = function::FunctionLowerer::entry("toplevel");
    // Function zero is the entry function. Reserve it before allocating nested
    // function identities for FunctionEntry constants.
    let _entry_id = module.fresh_function();
    let mut lowering = v2::Context::new(&mut module);
    let value = lowering.lower_expr(&mut entry, expr)?;
    entry.return_value(value)?;
    let mut entry_function = entry.into_function();
    entry_function.handler_regions = lowering.regions;
    Ok(Lowered {
        entry: entry_function,
        nested: module.into_functions(),
    })
}

mod v2 {
    use ncl_ir::{
        BlockId, Compare, Constant, Convert, FunctionId, HandlerKind, HandlerRegion,
        HandlerRegionId, OpKind, Param, Terminator, Ty, ValueId,
    };

    use crate::ast::{Expr, FunctionDesignator, LambdaExpr, Operator, TagbodyItem};
    use crate::lambda_list::{LambdaList, ParamName};
    use crate::symbols::SymbolRef;

    use super::capture;
    use super::env::{BlockEntry, FunctionEntry, Slot, TagEntry};
    use super::error::LowerError;
    use super::function::{FunctionLowerer, Lowerer};
    use super::literal::lower_literal;

    #[derive(Clone, Debug)]
    struct NonLocalTarget {
        name: SymbolRef,
        token: SymbolRef,
    }

    /// Per-function state not represented by the frozen `FunctionLowerer` API.
    pub(super) struct Context<'a> {
        pub(super) module: &'a mut Lowerer,
        pub(super) regions: Vec<HandlerRegion>,
        active: Vec<HandlerRegionId>,
        blocks: Vec<BlockId>,
        targets: Vec<NonLocalTarget>,
        next_region: u32,
    }

    impl<'a> Context<'a> {
        pub(super) fn new(module: &'a mut Lowerer) -> Self {
            Self::with_targets(module, Vec::new())
        }

        fn with_targets(module: &'a mut Lowerer, targets: Vec<NonLocalTarget>) -> Self {
            Self {
                module,
                regions: Vec::new(),
                active: Vec::new(),
                blocks: vec![BlockId(0)],
                targets,
                next_region: 0,
            }
        }

        fn block(&mut self, f: &mut FunctionLowerer, params: Vec<(Ty, ValueId)>) -> BlockId {
            let id = f.create_block(params);
            self.blocks.push(id);
            id
        }

        fn enter(
            &mut self,
            f: &mut FunctionLowerer,
            id: HandlerRegionId,
        ) -> Result<(), LowerError> {
            self.active.push(id);
            f.none(OpKind::EnterHandler { region: id })
        }

        fn leave(
            &mut self,
            f: &mut FunctionLowerer,
            id: HandlerRegionId,
        ) -> Result<(), LowerError> {
            f.none(OpKind::LeaveHandler { region: id })?;
            if self.active.last().copied() == Some(id) {
                self.active.pop();
            }
            Ok(())
        }

        fn token(name: &SymbolRef, kind: &str) -> SymbolRef {
            SymbolRef::interned("NCL-NLX", format!("{kind}:{}", name.name))
        }

        fn target(&self, name: &SymbolRef) -> Option<NonLocalTarget> {
            self.targets
                .iter()
                .rev()
                .find(|target| target.name == *name)
                .cloned()
        }

        pub(super) fn lower_expr(
            &mut self,
            f: &mut FunctionLowerer,
            expr: &Expr,
        ) -> Result<ValueId, LowerError> {
            match expr {
                Expr::Constant(literal) => lower_literal(f, literal),
                Expr::Variable(name) => Self::lower_variable(f, name),
                Expr::Call {
                    operator,
                    arguments,
                } => self.lower_call(f, operator, arguments),
                Expr::Function(designator) => self.lower_designator(f, designator),
                Expr::Lambda(lambda) => self.lower_lambda_value(f, lambda),
                Expr::If {
                    test,
                    then,
                    otherwise,
                } => self.lower_if(f, test, then, otherwise.as_deref()),
                Expr::Progn(forms) => self.lower_body(f, forms),
                Expr::Block { name, body } => self.lower_block(f, name, body),
                Expr::ReturnFrom { name, value } => {
                    self.lower_return_from(f, name, value.as_deref())
                }
                Expr::Tagbody(items) => self.lower_tagbody(f, items),
                Expr::Go { tag } => self.lower_go(f, tag),
                Expr::Catch { tag, body } => self.lower_catch(f, tag, body),
                Expr::Throw { tag, value } => self.lower_throw(f, tag, value),
                Expr::UnwindProtect { protected, cleanup } => {
                    self.lower_unwind(f, protected, cleanup)
                }
                Expr::Let {
                    sequential,
                    bindings,
                    body,
                    ..
                } => self.lower_let(f, *sequential, bindings, body),
                Expr::Progv {
                    symbols,
                    values,
                    body,
                } => self.lower_progv(f, symbols, values, body),
                Expr::Setq(pairs) => self.lower_setq(f, pairs),
                Expr::MultipleValueCall {
                    function,
                    arguments,
                } => self.lower_multiple_value_call(f, function, arguments),
                Expr::MultipleValueProg1 { first, forms } => {
                    let value = self.lower_expr(f, first)?;
                    for form in forms {
                        self.lower_expr(f, form)?;
                    }
                    f.none(OpKind::SetMultipleValues {
                        values: vec![value],
                    })?;
                    Ok(value)
                }
                Expr::The { value, .. } | Expr::LoadTimeValue { form: value, .. } => {
                    self.lower_expr(f, value)
                }
                Expr::EvalWhen { body, .. }
                | Expr::Locally { body, .. }
                | Expr::Macrolet { body, .. }
                | Expr::SymbolMacrolet { body, .. } => self.lower_body(f, body),
                Expr::Flet {
                    definitions, body, ..
                }
                | Expr::Labels {
                    definitions, body, ..
                } => self.lower_local_functions(f, definitions, body),
            }
        }

        fn lower_variable(
            f: &mut FunctionLowerer,
            name: &SymbolRef,
        ) -> Result<ValueId, LowerError> {
            match f.env().lookup_variable(name) {
                Some(Slot::Value(value)) => Ok(value),
                Some(Slot::Cell(address)) => f.one(OpKind::Load { address }, Ty::Word),
                None => {
                    let symbol = f.symbol(name)?;
                    f.safepoint()?;
                    f.one(
                        OpKind::Builtin {
                            name: "symbol-value".to_owned(),
                            args: vec![symbol],
                        },
                        Ty::Word,
                    )
                }
            }
        }

        fn lower_call(
            &mut self,
            f: &mut FunctionLowerer,
            operator: &Operator,
            arguments: &[Expr],
        ) -> Result<ValueId, LowerError> {
            let values = arguments
                .iter()
                .map(|argument| self.lower_expr(f, argument))
                .collect::<Result<Vec<_>, _>>()?;
            let (callee, closure) = match operator {
                Operator::Name(name) => match f.env().lookup_function(name) {
                    Some(entry) => (entry.callee, true),
                    None => (f.symbol(name)?, false),
                },
                Operator::Lambda(lambda) => (self.lower_lambda_value(f, lambda)?, true),
            };
            let argc_value = Self::argc(f, values.len())?;
            let mut args = vec![argc_value];
            args.extend(values);
            f.safepoint()?;
            if closure {
                f.one(
                    OpKind::CallClosure {
                        closure: callee,
                        args,
                    },
                    Ty::Word,
                )
            } else {
                f.one(
                    OpKind::Call {
                        function: callee,
                        args,
                    },
                    Ty::Word,
                )
            }
        }

        fn argc(f: &mut FunctionLowerer, count: usize) -> Result<ValueId, LowerError> {
            let raw = f.fixnum(i64::try_from(count).map_err(|_| LowerError::Ir {
                detail: "argument count does not fit i64".to_owned(),
            })?)?;
            f.one(
                OpKind::Convert {
                    op: Convert::I64ToWord,
                    value: raw,
                },
                Ty::Word,
            )
        }

        fn lower_designator(
            &mut self,
            f: &mut FunctionLowerer,
            designator: &FunctionDesignator,
        ) -> Result<ValueId, LowerError> {
            match designator {
                FunctionDesignator::Name(name) => f.symbol(name),
                FunctionDesignator::Lambda(lambda) => self.lower_lambda_value(f, lambda),
            }
        }

        fn lower_lambda_value(
            &mut self,
            f: &mut FunctionLowerer,
            lambda: &LambdaExpr,
        ) -> Result<ValueId, LowerError> {
            let captures = super::lambda::collect_captures(f, lambda);
            let id = self.lower_lambda(lambda, &captures)?;
            let entry = f.word_constant(Constant::FunctionEntry(id))?;
            let capture_values = captures
                .iter()
                .map(|(_, slot)| match slot {
                    Slot::Value(value) => Ok(*value),
                    Slot::Cell(address) => f.one(
                        OpKind::Convert {
                            op: Convert::AddressToWord,
                            value: *address,
                        },
                        Ty::Word,
                    ),
                })
                .collect::<Result<Vec<_>, LowerError>>()?;
            f.one(
                OpKind::MakeClosure {
                    entry,
                    captures: capture_values,
                },
                Ty::Word,
            )
        }

        fn lower_lambda(
            &mut self,
            lambda: &LambdaExpr,
            captures: &[super::lambda::Capture],
        ) -> Result<FunctionId, LowerError> {
            reject_lambda_list(&lambda.lambda_list)?;
            let id = self.module.fresh_function();
            let params = lambda_params(&lambda.lambda_list, captures)?;
            let mut nested =
                FunctionLowerer::new(id, format!("lambda-{id:?}"), params, vec![Ty::Word]);
            bind_captures(&mut nested, captures)?;
            bind_required(&mut nested, &lambda.lambda_list, 1 + captures.len())?;
            let mut child = Context::with_targets(self.module, self.targets.clone());
            child.bind_optional(&mut nested, &lambda.lambda_list, 1 + captures.len())?;
            child.bind_aux(&mut nested, &lambda.lambda_list)?;
            let value = child.lower_body(&mut nested, &lambda.body)?;
            if !nested.is_terminated() {
                nested.return_value(value)?;
            }
            let mut function = nested.into_function();
            function.handler_regions = child.regions;
            self.module.push_function(function);
            Ok(id)
        }

        fn lower_body(
            &mut self,
            f: &mut FunctionLowerer,
            forms: &[Expr],
        ) -> Result<ValueId, LowerError> {
            let mut value = None;
            for form in forms {
                if f.is_terminated() {
                    break;
                }
                value = Some(self.lower_expr(f, form)?);
            }
            value.map_or_else(|| f.nil(), Ok)
        }

        fn lower_if(
            &mut self,
            f: &mut FunctionLowerer,
            test: &Expr,
            then: &Expr,
            otherwise: Option<&Expr>,
        ) -> Result<ValueId, LowerError> {
            let tested = self.lower_expr(f, test)?;
            let nil = f.nil()?;
            let condition = f.one(
                OpKind::Compare {
                    op: Compare::Ne,
                    left: tested,
                    right: nil,
                },
                Ty::Bool,
            )?;
            let start = f.current_block();
            let result = f.fresh_value();
            let then_block = self.block(f, Vec::new());
            let else_block = self.block(f, Vec::new());
            let merge = self.block(f, vec![(Ty::Word, result)]);
            f.position(start)?;
            f.terminate(Terminator::Branch {
                condition,
                then_target: then_block,
                then_args: Vec::new(),
                else_target: else_block,
                else_args: Vec::new(),
            })?;
            f.position(then_block)?;
            let then_value = self.lower_expr(f, then)?;
            if !f.is_terminated() {
                f.terminate(Terminator::Jump {
                    target: merge,
                    args: vec![then_value],
                })?;
            }
            f.position(else_block)?;
            let else_value = match otherwise {
                Some(form) => self.lower_expr(f, form)?,
                None => f.nil()?,
            };
            if !f.is_terminated() {
                f.terminate(Terminator::Jump {
                    target: merge,
                    args: vec![else_value],
                })?;
            }
            f.position(merge)?;
            Ok(result)
        }

        fn lower_block(
            &mut self,
            f: &mut FunctionLowerer,
            name: &SymbolRef,
            body: &[Expr],
        ) -> Result<ValueId, LowerError> {
            if body_has_nested_return(body, name) {
                return self.lower_escaping_block(f, name, body);
            }
            let result = f.fresh_value();
            let start = f.current_block();
            let exit = self.block(f, vec![(Ty::Word, result)]);
            f.position(start)?;
            f.env().push();
            f.env().bind_block(BlockEntry {
                name: name.clone(),
                target: exit,
            });
            let value = self.lower_body(f, body)?;
            f.env().pop();
            if !f.is_terminated() {
                f.terminate(Terminator::Jump {
                    target: exit,
                    args: vec![value],
                })?;
            }
            f.position(exit)?;
            Ok(result)
        }

        fn lower_escaping_block(
            &mut self,
            f: &mut FunctionLowerer,
            name: &SymbolRef,
            body: &[Expr],
        ) -> Result<ValueId, LowerError> {
            let result = f.fresh_value();
            let start = f.current_block();
            let token_name = Context::token(name, "BLOCK");
            let token = f.symbol(&token_name)?;
            let region_id = HandlerRegionId(self.next_region);
            self.next_region += 1;
            self.enter(f, region_id)?;
            self.targets.push(NonLocalTarget {
                name: name.clone(),
                token: token_name,
            });
            let value = self.lower_body(f, body)?;
            self.targets.pop();
            let body_end = f.current_block();
            let normal_path = !f.is_terminated();
            let protected = self
                .blocks
                .clone()
                .into_iter()
                .filter(|block| block.0 >= start.0)
                .collect::<Vec<_>>();
            let exit = self.block(f, vec![(Ty::Word, result)]);
            if normal_path {
                f.position(body_end)?;
                self.leave(f, region_id)?;
                f.terminate(Terminator::Jump {
                    target: exit,
                    args: vec![value],
                })?;
            }
            let handler_value = f.fresh_value();
            let handler = self.block(f, vec![(Ty::Word, handler_value)]);
            self.leave(f, region_id)?;
            f.terminate(Terminator::Jump {
                target: exit,
                args: vec![handler_value],
            })?;
            self.regions.push(HandlerRegion {
                id: region_id,
                kind: HandlerKind::Catch,
                protected,
                handler,
                cleanup: None,
                catch_tag: Some(token),
                binding_targets: Vec::new(),
                depth: 0,
                parent: None,
            });
            f.position(exit)?;
            Ok(result)
        }

        fn lower_return_from(
            &mut self,
            f: &mut FunctionLowerer,
            name: &SymbolRef,
            value: Option<&Expr>,
        ) -> Result<ValueId, LowerError> {
            if let Some(entry) = f.env().lookup_block(name) {
                let value = match value {
                    Some(form) => self.lower_expr(f, form)?,
                    None => f.nil()?,
                };
                if !f.is_terminated() {
                    f.terminate(Terminator::Jump {
                        target: entry.target,
                        args: vec![value],
                    })?;
                }
                return Ok(value);
            }
            let target = self
                .target(name)
                .ok_or_else(|| LowerError::EscapingControl { name: name.clone() })?;
            let value = match value {
                Some(form) => self.lower_expr(f, form)?,
                None => f.nil()?,
            };
            let token = f.symbol(&target.token)?;
            f.safepoint()?;
            let thrown = f.one(
                OpKind::Builtin {
                    name: "throw".to_owned(),
                    args: vec![token, value],
                },
                Ty::Word,
            )?;
            if !f.is_terminated() {
                f.terminate(Terminator::Throw { condition: thrown })?;
            }
            Ok(value)
        }

        fn lower_tagbody(
            &mut self,
            f: &mut FunctionLowerer,
            items: &[TagbodyItem],
        ) -> Result<ValueId, LowerError> {
            let start = f.current_block();
            let exit = self.block(f, Vec::new());
            f.position(start)?;
            f.env().push();
            let mut targets = Vec::new();
            for item in items {
                if let TagbodyItem::Tag(tag) = item {
                    let block = self.block(f, Vec::new());
                    targets.push((tag.clone(), block));
                }
            }
            f.position(start)?;
            for (name, target) in &targets {
                f.env().bind_tag(TagEntry {
                    name: name.clone(),
                    target: *target,
                });
            }
            let mark = self.targets.len();
            for (name, _) in &targets {
                self.targets.push(NonLocalTarget {
                    name: name.clone(),
                    token: Context::token(name, "TAG"),
                });
            }
            let mut next = 0;
            for item in items {
                match item {
                    TagbodyItem::Tag(_) => {
                        let target =
                            targets.get(next).map(|(_, block)| *block).ok_or_else(|| {
                                LowerError::Ir {
                                    detail: "tagbody tag index out of bounds".to_owned(),
                                }
                            })?;
                        next += 1;
                        if !f.is_terminated() {
                            f.terminate(Terminator::Jump {
                                target,
                                args: Vec::new(),
                            })?;
                        }
                        f.position(target)?;
                    }
                    TagbodyItem::Form(form) => {
                        if !f.is_terminated() {
                            self.lower_expr(f, form)?;
                        }
                    }
                }
            }
            self.targets.truncate(mark);
            if !f.is_terminated() {
                f.terminate(Terminator::Jump {
                    target: exit,
                    args: Vec::new(),
                })?;
            }
            f.position(exit)?;
            f.env().pop();
            f.nil()
        }

        fn lower_go(
            &self,
            f: &mut FunctionLowerer,
            tag: &SymbolRef,
        ) -> Result<ValueId, LowerError> {
            if let Some(entry) = f.env().lookup_tag(tag) {
                let value = f.nil()?;
                if !f.is_terminated() {
                    f.terminate(Terminator::Jump {
                        target: entry.target,
                        args: Vec::new(),
                    })?;
                }
                return Ok(value);
            }
            let target = self
                .target(tag)
                .ok_or_else(|| LowerError::EscapingControl { name: tag.clone() })?;
            let value = f.nil()?;
            let token = f.symbol(&target.token)?;
            f.safepoint()?;
            let thrown = f.one(
                OpKind::Builtin {
                    name: "throw".to_owned(),
                    args: vec![token, value],
                },
                Ty::Word,
            )?;
            if !f.is_terminated() {
                f.terminate(Terminator::Throw { condition: thrown })?;
            }
            Ok(value)
        }

        fn lower_catch(
            &mut self,
            f: &mut FunctionLowerer,
            tag: &Expr,
            body: &[Expr],
        ) -> Result<ValueId, LowerError> {
            let tag = self.lower_expr(f, tag)?;
            let result = f.fresh_value();
            let region_id = HandlerRegionId(self.next_region);
            self.next_region += 1;
            let start = f.current_block();
            self.enter(f, region_id)?;
            let value = self.lower_body(f, body)?;
            let protected = self
                .blocks
                .iter()
                .copied()
                .filter(|block| block.0 >= start.0)
                .collect::<Vec<_>>();
            let body_end = f.current_block();
            let normal_path = !f.is_terminated();
            let exit = self.block(f, vec![(Ty::Word, result)]);
            if normal_path {
                f.position(body_end)?;
                self.leave(f, region_id)?;
                f.terminate(Terminator::Jump {
                    target: exit,
                    args: vec![value],
                })?;
            }
            let caught = f.fresh_value();
            let handler = self.block(f, vec![(Ty::Word, caught)]);
            self.leave(f, region_id)?;
            f.terminate(Terminator::Jump {
                target: exit,
                args: vec![caught],
            })?;
            self.regions.push(HandlerRegion {
                id: region_id,
                kind: HandlerKind::Catch,
                protected,
                handler,
                cleanup: None,
                catch_tag: Some(tag),
                binding_targets: Vec::new(),
                depth: 0,
                parent: None,
            });
            f.position(exit)?;
            Ok(result)
        }

        fn lower_throw(
            &mut self,
            f: &mut FunctionLowerer,
            tag: &Expr,
            value: &Expr,
        ) -> Result<ValueId, LowerError> {
            let tag = self.lower_expr(f, tag)?;
            let value = self.lower_expr(f, value)?;
            f.safepoint()?;
            let thrown = f.one(
                OpKind::Builtin {
                    name: "throw".to_owned(),
                    args: vec![tag, value],
                },
                Ty::Word,
            )?;
            if !f.is_terminated() {
                f.terminate(Terminator::Throw { condition: thrown })?;
            }
            Ok(value)
        }

        fn lower_unwind(
            &mut self,
            f: &mut FunctionLowerer,
            protected_form: &Expr,
            cleanup: &[Expr],
        ) -> Result<ValueId, LowerError> {
            let result = f.fresh_value();
            let region_id = HandlerRegionId(self.next_region);
            self.next_region += 1;
            let start = f.current_block();
            self.enter(f, region_id)?;
            let value = self.lower_expr(f, protected_form)?;
            let protected = self
                .blocks
                .iter()
                .copied()
                .filter(|block| block.0 >= start.0)
                .collect::<Vec<_>>();
            let body_end = f.current_block();
            let normal_path = !f.is_terminated();
            let merge = self.block(f, vec![(Ty::Word, result)]);
            if normal_path {
                f.position(body_end)?;
                self.leave(f, region_id)?;
                f.terminate(Terminator::Jump {
                    target: merge,
                    args: vec![value],
                })?;
            }
            let cleanup_block = self.block(f, Vec::new());
            for form in cleanup {
                if !f.is_terminated() {
                    self.lower_expr(f, form)?;
                }
            }
            if !f.is_terminated() {
                self.leave(f, region_id)?;
                let cleanup_value = f.nil()?;
                f.terminate(Terminator::Return {
                    values: vec![cleanup_value],
                })?;
            }
            self.regions.push(HandlerRegion {
                id: region_id,
                kind: HandlerKind::UnwindProtect,
                protected,
                handler: cleanup_block,
                cleanup: Some(cleanup_block),
                catch_tag: None,
                binding_targets: Vec::new(),
                depth: 0,
                parent: None,
            });
            f.position(merge)?;
            Ok(result)
        }

        fn lower_progv(
            &mut self,
            f: &mut FunctionLowerer,
            symbols: &Expr,
            values: &Expr,
            body: &[Expr],
        ) -> Result<ValueId, LowerError> {
            let symbols = self.lower_expr(f, symbols)?;
            let values = self.lower_expr(f, values)?;
            let result = f.fresh_value();
            let region_id = HandlerRegionId(self.next_region);
            self.next_region += 1;
            let start = f.current_block();
            self.enter(f, region_id)?;
            let value = self.lower_body(f, body)?;
            let protected = self
                .blocks
                .iter()
                .copied()
                .filter(|block| block.0 >= start.0)
                .collect::<Vec<_>>();
            let body_end = f.current_block();
            let normal_path = !f.is_terminated();
            let merge = self.block(f, vec![(Ty::Word, result)]);
            if normal_path {
                f.position(body_end)?;
                self.leave(f, region_id)?;
                f.terminate(Terminator::Jump {
                    target: merge,
                    args: vec![value],
                })?;
            }
            let handler = self.block(f, Vec::new());
            self.leave(f, region_id)?;
            let restored_value = f.nil()?;
            f.terminate(Terminator::Return {
                values: vec![restored_value],
            })?;
            self.regions.push(HandlerRegion {
                id: region_id,
                kind: HandlerKind::Progv,
                protected,
                handler,
                cleanup: None,
                catch_tag: None,
                binding_targets: vec![symbols, values],
                depth: 0,
                parent: None,
            });
            f.position(merge)?;
            Ok(result)
        }

        fn lower_let(
            &mut self,
            f: &mut FunctionLowerer,
            sequential: bool,
            bindings: &[crate::ast::LetBinding],
            body: &[Expr],
        ) -> Result<ValueId, LowerError> {
            let analysis = capture::analyze(body);
            f.env().push();
            if sequential {
                for binding in bindings {
                    let value = match binding.value.as_ref() {
                        Some(form) => self.lower_expr(f, form)?,
                        None => f.nil()?,
                    };
                    bind_let(f, &binding.name, value, &analysis)?;
                }
            } else {
                let mut evaluated = Vec::new();
                for binding in bindings {
                    let value = match binding.value.as_ref() {
                        Some(form) => self.lower_expr(f, form)?,
                        None => f.nil()?,
                    };
                    evaluated.push((binding.name.clone(), value));
                }
                for (name, value) in evaluated {
                    bind_let(f, &name, value, &analysis)?;
                }
            }
            let value = self.lower_body(f, body)?;
            f.env().pop();
            Ok(value)
        }

        fn lower_setq(
            &mut self,
            f: &mut FunctionLowerer,
            pairs: &[(SymbolRef, Expr)],
        ) -> Result<ValueId, LowerError> {
            let mut last = None;
            for (name, form) in pairs {
                let value = self.lower_expr(f, form)?;
                match f.env().lookup_variable(name) {
                    Some(Slot::Cell(address)) => f.none(OpKind::Store { address, value })?,
                    Some(Slot::Value(_)) => {
                        f.env().rebind_variable(name, Slot::Value(value));
                    }
                    None => {
                        let symbol = f.symbol(name)?;
                        f.safepoint()?;
                        f.one(
                            OpKind::Builtin {
                                name: "set-symbol-value".to_owned(),
                                args: vec![symbol, value],
                            },
                            Ty::Word,
                        )?;
                    }
                }
                last = Some(value);
            }
            last.map_or_else(|| f.nil(), Ok)
        }

        fn lower_multiple_value_call(
            &mut self,
            f: &mut FunctionLowerer,
            function: &Expr,
            arguments: &[Expr],
        ) -> Result<ValueId, LowerError> {
            let callee = self.lower_expr(f, function)?;
            let mut args = vec![callee];
            for argument in arguments {
                args.push(self.lower_expr(f, argument)?);
            }
            f.safepoint()?;
            f.one(
                OpKind::Builtin {
                    name: "multiple-value-call".to_owned(),
                    args,
                },
                Ty::Word,
            )
        }

        fn lower_local_functions(
            &mut self,
            f: &mut FunctionLowerer,
            definitions: &[crate::ast::LocalFunction],
            body: &[Expr],
        ) -> Result<ValueId, LowerError> {
            f.env().push();
            for definition in definitions {
                let captures = super::lambda::collect_captures(f, &definition.lambda);
                let closure = self.lower_lambda_value(f, &definition.lambda)?;
                let _ = captures;
                f.env().bind_function(FunctionEntry {
                    name: definition.name.clone(),
                    callee: closure,
                });
            }
            let value = self.lower_body(f, body)?;
            f.env().pop();
            Ok(value)
        }

        fn bind_optional(
            &mut self,
            f: &mut FunctionLowerer,
            list: &LambdaList,
            base: usize,
        ) -> Result<(), LowerError> {
            for (offset, optional) in list.optional.iter().enumerate() {
                let name = param_name(&optional.name)?;
                let argc_word = f.one(OpKind::LoadArg { index: 0 }, Ty::Word)?;
                let argc = f.one(
                    OpKind::Convert {
                        op: Convert::WordToI64,
                        value: argc_word,
                    },
                    Ty::I64,
                )?;
                let threshold = f.fixnum(
                    i64::try_from(list.required.len() + offset + 1).map_err(|_| {
                        LowerError::Ir {
                            detail: "optional parameter index does not fit i64".to_owned(),
                        }
                    })?,
                )?;
                let present = f.one(
                    OpKind::Compare {
                        op: Compare::Ge,
                        left: argc,
                        right: threshold,
                    },
                    Ty::Bool,
                )?;
                let branch = f.current_block();
                let value_slot = f.fresh_value();
                let flag_slot = f.fresh_value();
                let supplied = self.block(f, Vec::new());
                let default = self.block(f, Vec::new());
                let merge = self.block(f, vec![(Ty::Word, value_slot), (Ty::Word, flag_slot)]);
                f.position(branch)?;
                f.terminate(Terminator::Branch {
                    condition: present,
                    then_target: supplied,
                    then_args: Vec::new(),
                    else_target: default,
                    else_args: Vec::new(),
                })?;
                f.position(default)?;
                let default_value = match optional.default.as_ref() {
                    Some(form) => self.lower_expr(f, form)?,
                    None => f.nil()?,
                };
                let absent = f.fixnum(0)?;
                let absent = f.one(
                    OpKind::Convert {
                        op: Convert::I64ToWord,
                        value: absent,
                    },
                    Ty::Word,
                )?;
                f.terminate(Terminator::Jump {
                    target: merge,
                    args: vec![default_value, absent],
                })?;
                f.position(supplied)?;
                let parameter = param_index(base + list.required.len() + offset)?;
                let supplied_value = f.one(OpKind::LoadArg { index: parameter }, Ty::Word)?;
                let present_flag = f.fixnum(1)?;
                let present_flag = f.one(
                    OpKind::Convert {
                        op: Convert::I64ToWord,
                        value: present_flag,
                    },
                    Ty::Word,
                )?;
                f.terminate(Terminator::Jump {
                    target: merge,
                    args: vec![supplied_value, present_flag],
                })?;
                f.position(merge)?;
                f.env().bind_variable(name, Slot::Value(value_slot));
                if let Some(supplied_p) = &optional.supplied_p {
                    f.env()
                        .bind_variable(param_name(supplied_p)?, Slot::Value(flag_slot));
                }
            }
            Ok(())
        }

        fn bind_aux(
            &mut self,
            f: &mut FunctionLowerer,
            list: &LambdaList,
        ) -> Result<(), LowerError> {
            for aux in &list.aux {
                let value = match aux.default.as_ref() {
                    Some(form) => self.lower_expr(f, form)?,
                    None => f.nil()?,
                };
                f.env()
                    .bind_variable(param_name(&aux.name)?, Slot::Value(value));
            }
            Ok(())
        }
    }

    const fn reject_lambda_list(list: &LambdaList) -> Result<(), LowerError> {
        if list.rest.is_some() {
            return Err(LowerError::UnsupportedLambdaList { feature: "&rest" });
        }
        if !list.keys.is_empty() || list.allow_other_keys {
            return Err(LowerError::UnsupportedLambdaList { feature: "&key" });
        }
        if list.whole.is_some() || list.environment.is_some() || list.body.is_some() {
            return Err(LowerError::UnsupportedLambdaList {
                feature: "macro lambda list keyword",
            });
        }
        Ok(())
    }

    fn param_name(name: &ParamName) -> Result<SymbolRef, LowerError> {
        match name {
            ParamName::Symbol(symbol) => Ok(symbol.clone()),
            ParamName::Pattern(_) => Err(LowerError::UnsupportedLambdaList {
                feature: "destructuring parameter",
            }),
        }
    }

    fn lambda_params(
        list: &LambdaList,
        captures: &[super::lambda::Capture],
    ) -> Result<Vec<Param>, LowerError> {
        let mut params = vec![Param {
            name: "argc".to_owned(),
            ty: Ty::Word,
        }];
        for (name, _) in captures {
            params.push(Param {
                name: name.name.clone(),
                ty: Ty::Word,
            });
        }
        for name in &list.required {
            params.push(Param {
                name: param_name(name)?.name,
                ty: Ty::Word,
            });
        }
        for optional in &list.optional {
            params.push(Param {
                name: param_name(&optional.name)?.name,
                ty: Ty::Word,
            });
        }
        Ok(params)
    }

    fn bind_captures(
        f: &mut FunctionLowerer,
        captures: &[super::lambda::Capture],
    ) -> Result<(), LowerError> {
        for (index, (name, slot)) in captures.iter().enumerate() {
            let parameter = param_index(1 + index)?;
            let loaded = f.one(OpKind::LoadArg { index: parameter }, Ty::Word)?;
            match slot {
                Slot::Value(_) => f.env().bind_variable(name.clone(), Slot::Value(loaded)),
                Slot::Cell(_) => {
                    let address = f.one(
                        OpKind::Convert {
                            op: Convert::WordToAddress,
                            value: loaded,
                        },
                        Ty::Address,
                    )?;
                    f.env().bind_variable(name.clone(), Slot::Cell(address));
                }
            }
        }
        Ok(())
    }

    fn bind_required(
        f: &mut FunctionLowerer,
        list: &LambdaList,
        base: usize,
    ) -> Result<(), LowerError> {
        for (offset, name) in list.required.iter().enumerate() {
            let loaded = f.one(
                OpKind::LoadArg {
                    index: param_index(base + offset)?,
                },
                Ty::Word,
            )?;
            f.env()
                .bind_variable(param_name(name)?, Slot::Value(loaded));
        }
        Ok(())
    }

    fn bind_let(
        f: &mut FunctionLowerer,
        name: &SymbolRef,
        value: ValueId,
        analysis: &capture::Analysis,
    ) -> Result<(), LowerError> {
        if analysis.needs_cell(name) {
            let address = f.one(OpKind::Alloc { words: 1 }, Ty::Address)?;
            f.safepoint()?;
            f.none(OpKind::Store { address, value })?;
            f.env().bind_variable(name.clone(), Slot::Cell(address));
        } else {
            f.env().bind_variable(name.clone(), Slot::Value(value));
        }
        Ok(())
    }

    fn param_index(index: usize) -> Result<u8, LowerError> {
        u8::try_from(index).map_err(|_| LowerError::Ir {
            detail: "parameter index exceeds 255".to_owned(),
        })
    }

    fn body_has_nested_return(forms: &[Expr], name: &SymbolRef) -> bool {
        forms.iter().any(|form| nested_return(form, name))
    }

    fn nested_return(expr: &Expr, name: &SymbolRef) -> bool {
        match expr {
            Expr::Lambda(lambda) => lambda.body.iter().any(|form| contains_return(form, name)),
            Expr::Call {
                operator,
                arguments,
            } => {
                matches!(operator, Operator::Lambda(lambda) if lambda.body.iter().any(|form| contains_return(form, name)))
                    || arguments.iter().any(|form| nested_return(form, name))
            }
            Expr::Function(FunctionDesignator::Lambda(lambda)) => {
                lambda.body.iter().any(|form| contains_return(form, name))
            }
            Expr::Let { bindings, body, .. } => {
                bindings
                    .iter()
                    .filter_map(|binding| binding.value.as_ref())
                    .any(|form| nested_return(form, name))
                    || body.iter().any(|form| nested_return(form, name))
            }
            Expr::Progn(forms)
            | Expr::Locally { body: forms, .. }
            | Expr::Macrolet { body: forms, .. }
            | Expr::SymbolMacrolet { body: forms, .. } => {
                forms.iter().any(|form| nested_return(form, name))
            }
            _ => false,
        }
    }

    fn contains_return(expr: &Expr, name: &SymbolRef) -> bool {
        match expr {
            Expr::ReturnFrom { name: target, .. } => target == name,
            Expr::Progn(forms)
            | Expr::Locally { body: forms, .. }
            | Expr::Macrolet { body: forms, .. }
            | Expr::SymbolMacrolet { body: forms, .. } => {
                forms.iter().any(|form| contains_return(form, name))
            }
            Expr::Call { arguments, .. } => {
                arguments.iter().any(|form| contains_return(form, name))
            }
            _ => false,
        }
    }
}
