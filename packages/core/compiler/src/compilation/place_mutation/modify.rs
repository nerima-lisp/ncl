#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
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
        self.emit(function, Instruction::Primary, place.span)?;
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

    pub(crate) fn compile_modify_place(
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
        let place = &items[1];
        if Self::symbol_name_info(place, &format!("{operator} target")).is_ok() {
            return self.compile_modify_symbol(function, span, items, operator, arithmetic);
        }
        if !stable_list_place(place) {
            return self.compile_runtime_definition(function, span, items);
        }
        let FormKind::List(place_items) = &place.kind else {
            unreachable!("stable list place was checked above");
        };
        let place_operator =
            Self::symbol_name_info(&place_items[0], "modify list place operator")?.0;
        self.compile_expression(function, &place_items[1])?;
        self.emit(function, Instruction::Dup, place.span)?;
        self.emit(
            function,
            Instruction::FunctionLoad(place_operator),
            place.span,
        )?;
        self.emit(function, Instruction::Swap, place.span)?;
        self.emit(function, Instruction::Call(1), place.span)?;
        self.emit(function, Instruction::Primary, place.span)?;
        self.emit(
            function,
            Instruction::FunctionLoad(arithmetic.to_string()),
            place.span,
        )?;
        self.emit(function, Instruction::Swap, place.span)?;
        if let Some(delta) = items.get(2) {
            self.compile_expression(function, delta)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Integer(1)), span)?;
        }
        self.emit(function, Instruction::Call(2), span)?;
        let operator = Self::symbol_name_info(&place_items[0], "modify list place operator")?.0;
        self.emit(
            function,
            Instruction::SetfListPlace {
                operator,
                place: place_items[1].clone(),
            },
            place.span,
        )?;
        Ok(())
    }
}
