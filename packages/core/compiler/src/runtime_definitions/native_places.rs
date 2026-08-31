#[allow(clippy::wildcard_imports)]
use super::super::*;

impl CompileState {
    pub(super) fn compile_native_rotate_shift(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<Option<()>, CompileError> {
        let Some((operator, _)) = items
            .first()
            .and_then(|form| Self::symbol_name_info(form, "runtime operator").ok())
        else {
            return Ok(None);
        };
        if operator != "ROTATEF" && operator != "SHIFTF" {
            return Ok(None);
        }
        let place_count = if operator == "ROTATEF" {
            items.len().saturating_sub(1)
        } else {
            items.len().saturating_sub(2)
        };
        if operator == "SHIFTF" && place_count < 1 {
            return Err(Self::arity_error(items, &operator, "at least one", span));
        }
        let place_forms = if operator == "ROTATEF" {
            &items[1..]
        } else {
            &items[1..items.len() - 1]
        };
        let places = place_forms
            .iter()
            .map(|place| Self::symbol_name_info(place, "symbol place"))
            .collect::<Result<Vec<_>, _>>()
            .ok();
        let Some(places) = places else {
            return Ok(None);
        };
        for place in place_forms {
            self.compile_expression(function, place)?;
        }
        if operator == "SHIFTF" {
            self.compile_expression(function, &items[items.len() - 1])?;
        }
        self.emit(
            function,
            if operator == "ROTATEF" {
                Instruction::RotatefSymbols(places)
            } else {
                Instruction::ShiftfSymbols(places)
            },
            items[0].span,
        )?;
        Ok(Some(()))
    }

    pub(super) fn compile_native_push_pop(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<Option<()>, CompileError> {
        let Some(operator) = items
            .first()
            .and_then(|form| Self::symbol_name_info(form, "runtime operator").ok())
            .map(|(name, _)| name)
        else {
            return Ok(None);
        };
        if !matches!(operator.as_str(), "PUSH" | "POP" | "PUSHNEW") {
            return Ok(None);
        }
        let expected = if operator == "POP" { 2 } else { 3 };
        if operator == "PUSHNEW" && items.len() > expected {
            let Some((name, escaped)) = Self::symbol_name_info(&items[2], "list place").ok() else {
                return Ok(None);
            };
            if !(items.len() - 3).is_multiple_of(2) {
                return Ok(None);
            }
            let mut test_not = false;
            let mut has_test = false;
            let mut has_key = false;
            let mut key_before_test = false;
            for pair in items[3..].chunks_exact(2) {
                let FormKind::Atom(keyword) = &pair[0].kind else {
                    return Ok(None);
                };
                let keyword = keyword.to_ascii_uppercase();
                if !keyword.starts_with(':') {
                    return Ok(None);
                }
                match keyword.as_str() {
                    ":TEST" if !has_test && !test_not => has_test = true,
                    ":TEST-NOT" if !has_test && !test_not => test_not = true,
                    ":KEY" if !has_key => {
                        key_before_test = !has_test && !test_not;
                        has_key = true;
                    }
                    _ => return Ok(None),
                }
                self.compile_expression(function, &pair[1])?;
            }
            if !has_test && !test_not {
                self.emit(
                    function,
                    Instruction::Quote(Form::atom("EQL", items[0].span)),
                    items[0].span,
                )?;
            }
            self.compile_expression(function, &items[1])?;
            self.compile_expression(function, &items[2])?;
            self.emit(
                function,
                Instruction::PushNewListOptions {
                    name,
                    escaped,
                    test_not,
                    has_key,
                    key_before_test,
                },
                items[0].span,
            )?;
            return Ok(Some(()));
        }
        if items.len() != expected {
            return Err(Self::arity_error(
                items,
                &operator,
                if operator == "PUSH" { "two" } else { "one" },
                span,
            ));
        }
        let Some((name, escaped)) = Self::symbol_name_info(&items[expected - 1], "list place").ok()
        else {
            return Ok(None);
        };
        if matches!(operator.as_str(), "PUSH" | "PUSHNEW") {
            self.compile_expression(function, &items[1])?;
        }
        self.compile_expression(function, &items[expected - 1])?;
        self.emit(
            function,
            match operator.as_str() {
                "PUSH" => Instruction::PushList { name, escaped },
                "PUSHNEW" => Instruction::PushNewList { name, escaped },
                _ => Instruction::PopList { name, escaped },
            },
            items[0].span,
        )?;
        Ok(Some(()))
    }
}
