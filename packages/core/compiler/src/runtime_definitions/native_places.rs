#[allow(clippy::wildcard_imports)]
use super::super::*;

fn generalized_list_place(form: &Form) -> Option<(String, String, bool)> {
    let FormKind::List(items) = &form.kind else {
        return None;
    };
    if items.len() != 2 {
        return None;
    }
    let (accessor, _) = CompileState::symbol_name_info(&items[0], "list accessor").ok()?;
    if !matches!(accessor.as_str(), "CAR" | "FIRST" | "CDR" | "REST") {
        return None;
    }
    let (name, escaped) = CompileState::symbol_name_info(&items[1], "list place target").ok()?;
    Some((accessor, name, escaped))
}

impl CompileState {
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
        let place = &items[expected - 1];
        let generalized = generalized_list_place(place);
        let symbol_place = Self::symbol_name_info(place, "list place").ok();
        if generalized.is_none() && symbol_place.is_none() {
            return Ok(None);
        }
        if matches!(operator.as_str(), "PUSH" | "PUSHNEW") {
            self.compile_expression(function, &items[1])?;
        }
        if generalized.is_some() {
            if let FormKind::List(place_items) = &place.kind {
                self.compile_expression(function, &place_items[1])?;
            }
        } else {
            self.compile_expression(function, place)?;
        }
        self.emit(
            function,
            if let Some((accessor, name, escaped)) = generalized {
                Instruction::ListPlaceMutation {
                    operator,
                    accessor,
                    name,
                    escaped,
                }
            } else {
                let (name, escaped) = symbol_place.expect("checked above");
                match operator.as_str() {
                    "PUSH" => Instruction::PushList { name, escaped },
                    "PUSHNEW" => Instruction::PushNewList { name, escaped },
                    _ => Instruction::PopList { name, escaped },
                }
            },
            items[0].span,
        )?;
        Ok(Some(()))
    }
}
