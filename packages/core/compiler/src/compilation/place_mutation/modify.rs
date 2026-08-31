#![allow(clippy::wildcard_imports)]
use super::super::*;

impl CompileState {
    fn generalized_list_place(form: &Form) -> Option<(Vec<String>, String, bool)> {
        let FormKind::List(items) = &form.kind else {
            return None;
        };
        if items.len() != 2 {
            return None;
        }
        let (accessor, _) = Self::symbol_name_info(&items[0], "modify accessor").ok()?;
        if !matches!(accessor.as_str(), "CAR" | "FIRST" | "CDR" | "REST") {
            return None;
        }
        if let Some((mut accessors, name, escaped)) = Self::generalized_list_place(&items[1]) {
            accessors.insert(0, accessor);
            return Some((accessors, name, escaped));
        }
        let (name, escaped) = Self::symbol_name_info(&items[1], "modify place target").ok()?;
        Some((vec![accessor], name, escaped))
    }

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
        if let Some((accessors, name, escaped)) = Self::generalized_list_place(place) {
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
