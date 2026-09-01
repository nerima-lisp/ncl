#![allow(clippy::wildcard_imports)]
use super::super::*;

impl CompileState {
    pub(crate) fn compile_modify(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operator: &str,
        arithmetic: &str,
    ) -> Result<(), CompileError> {
        let place = items
            .get(1)
            .ok_or_else(|| Self::internal_error(span, "missing modifying place"))?;
        if self.compile_modify_get_place(function, span, items, operator, arithmetic)? {
            return Ok(());
        }
        if let Some((place_items, accessor, name, escaped)) = modify_aref_place(place) {
            if !(items.len() == 2 || items.len() == 3) {
                return Err(Self::arity_error(items, operator, "one or two", span));
            }
            self.compile_expression(function, &place_items[1])?;
            for index_form in &place_items[2..] {
                self.compile_expression(function, index_form)?;
            }
            if let Some(delta) = items.get(2) {
                self.compile_expression(function, delta)?;
            } else {
                self.emit(function, Instruction::Constant(Constant::Integer(1)), span)?;
            }
            self.emit(function, Instruction::ModifyArefDynamic { rank: place_items.len() - 2, arithmetic: arithmetic.to_string(), operator: accessor, name, escaped }, place.span)?;
            return Ok(());
        }
        if let Some((index_form, target, name, escaped)) = modify_nth_place(place) {
            if !(items.len() == 2 || items.len() == 3) {
                return Err(Self::arity_error(items, operator, "one or two", span));
            }
            self.compile_expression(function, index_form)?;
            self.compile_expression(function, target)?;
            if let Some(delta) = items.get(2) {
                self.compile_expression(function, delta)?;
            } else {
                self.emit(function, Instruction::Constant(Constant::Integer(1)), span)?;
            }
            self.emit(function, Instruction::ModifyNthDynamic { arithmetic: arithmetic.to_string(), name, escaped }, place.span)?;
            return Ok(());
        }
        if let Some((accessors, name, escaped)) = generalized_list_place(place) {
            if !(items.len() == 2 || items.len() == 3) {
                return Err(Self::arity_error(items, operator, "one or two", span));
            }
            self.emit(function, Instruction::Load(name.clone()), place.span)?;
            self.emit(
                function,
                Instruction::FunctionLoad(arithmetic.to_string()),
                span,
            )?;
            self.compile_expression(function, place)?;
            if let Some(delta) = items.get(2) {
                self.compile_expression(function, delta)?;
            } else {
                self.emit(function, Instruction::Constant(Constant::Integer(1)), span)?;
            }
            self.emit(function, Instruction::Call(2), span)?;
            self.emit(
                function,
                Instruction::SetfNestedList {
                    accessors,
                    name,
                    escaped,
                },
                place.span,
            )?;
            return Ok(());
        }
        self.compile_modify_symbol(function, span, items, operator, arithmetic)
    }

    fn compile_modify_get_place(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operator: &str,
        arithmetic: &str,
    ) -> Result<bool, CompileError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(Self::arity_error(items, operator, "one or two", span));
        }
        let Some(FormKind::List(place_items)) = items.get(1).map(|form| &form.kind) else {
            return Ok(false);
        };
        if place_items.len() != 3
            || Self::symbol_name_info(&place_items[0], "modify place operator")
                .ok()
                .is_none_or(|(name, _)| name != "GET")
        {
            return Ok(false);
        }
        self.compile_expression(function, &place_items[1])?;
        self.compile_expression(function, &place_items[2])?;
        if let Some(delta) = items.get(2) {
            self.compile_expression(function, delta)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Integer(1)), span)?;
        }
        self.emit(
            function,
            Instruction::ModifyGetDynamic {
                arithmetic: arithmetic.to_string(),
            },
            items[1].span,
        )?;
        Ok(true)
    }

    pub(crate) fn compile_modify_symbol(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operator: &str,
        arithmetic: &str,
    ) -> Result<(), CompileError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(Self::arity_error(items, operator, "one or two", span));
        }
        let place = items
            .get(1)
            .ok_or_else(|| Self::internal_error(span, "missing modifying place"))?;
        let (name, escaped) = Self::symbol_name_info(place, &format!("{operator} target"))?;
        self.emit(
            function,
            Instruction::FunctionLoad(arithmetic.to_string()),
            place.span,
        )?;
        self.compile_expression(function, place)?;
        if let Some(delta) = items.get(2) {
            self.compile_expression(function, delta)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Integer(1)), span)?;
        }
        self.emit(function, Instruction::Call(2), span)?;
        self.emit(
            function,
            if escaped {
                Instruction::SetExact(name)
            } else {
                Instruction::Set(name)
            },
            place.span,
        )?;
        Ok(())
    }
}

fn modify_aref_place(place: &Form) -> Option<(&[Form], String, String, bool)> {
    let FormKind::List(items) = &place.kind else { return None };
    if items.len() < 3 { return None; }
    let (operator, _) = CompileState::symbol_name_info(&items[0], "modify place operator").ok()?;
    if !matches!(operator.as_str(), "AREF" | "SVREF" | "ROW-MAJOR-AREF") { return None; }
    let (name, escaped) = CompileState::symbol_name_info(&items[1], "modify aref target").ok()?;
    Some((items, operator, name, escaped))
}

fn modify_nth_place(place: &Form) -> Option<(&Form, &Form, String, bool)> {
    let FormKind::List(items) = &place.kind else { return None };
    if items.len() != 3 || CompileState::symbol_name_info(&items[0], "modify place operator").ok()?.0 != "NTH" { return None; }
    let (name, escaped) = CompileState::symbol_name_info(&items[2], "modify nth target").ok()?;
    Some((&items[1], &items[2], name, escaped))
}
