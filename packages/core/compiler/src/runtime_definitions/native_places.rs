#[allow(clippy::wildcard_imports)]
use super::super::*;

struct PushNewOptions {
    test_not: bool,
    has_key: bool,
    key_before_test: bool,
}

impl CompileState {
    fn compile_pushnew_options(
        &mut self,
        function: FunctionId,
        span: Span,
        options: &[Form],
    ) -> Result<Option<PushNewOptions>, CompileError> {
        if !options.len().is_multiple_of(2) {
            return Ok(None);
        }
        let mut test_not = false;
        let mut has_test = false;
        let mut has_key = false;
        let mut key_before_test = false;
        for pair in options.chunks_exact(2) {
            let FormKind::Atom(keyword) = &pair[0].kind else {
                return Ok(None);
            };
            match keyword.to_ascii_uppercase().as_str() {
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
            self.emit(function, Instruction::Quote(Form::atom("EQL", span)), span)?;
        }
        Ok(Some(PushNewOptions { test_not, has_key, key_before_test }))
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
        if operator == "PUSHNEW"
            && items.len() > expected
            && nth_list_place(&items[2]).is_some()
        {
            let Some(options) = self.compile_pushnew_options(function, items[0].span, &items[3..])? else { return Ok(None); };
            let (index_form, target, name, escaped) = nth_list_place(&items[2]).expect("checked above");
            self.compile_expression(function, &items[1])?;
            self.compile_expression(function, index_form)?;
            self.compile_expression(function, target)?;
            self.emit(function, Instruction::ListMutationNthPushNewOptions { name, escaped, test_not: options.test_not, has_key: options.has_key, key_before_test: options.key_before_test }, items[0].span)?;
            return Ok(Some(()));
        }
        if operator == "PUSHNEW"
            && items.len() > expected
            && (generalized_list_place(&items[2]).is_some()
                || Self::symbol_name_info(&items[2], "list place").is_ok())
        {
            let generalized = generalized_list_place(&items[2]);
            let symbol_place = Self::symbol_name_info(&items[2], "list place").ok();
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
            if generalized.is_some() {
                let FormKind::List(place_items) = &items[2].kind else {
                    unreachable!()
                };
                let base = generalized.as_ref().expect("checked above").1.clone();
                self.emit(function, Instruction::Load(base), place_items[1].span)?;
            } else {
                self.compile_expression(function, &items[2])?;
            }
            self.emit(
                function,
                if let Some((accessors, name, escaped)) = generalized {
                    if accessors.len() > 1 {
                        Instruction::NestedListPlacePushNewOptions {
                            accessors,
                            name,
                            escaped,
                            test_not,
                            has_key,
                            key_before_test,
                        }
                    } else {
                        Instruction::ListPlacePushNewOptions {
                            accessor: accessors[0].clone(),
                            name,
                            escaped,
                            test_not,
                            has_key,
                            key_before_test,
                        }
                    }
                } else {
                    let (name, escaped) = symbol_place.expect("checked above");
                    Instruction::PushNewListOptions {
                        name,
                        escaped,
                        test_not,
                        has_key,
                        key_before_test,
                    }
                },
                items[0].span,
            )?;
            return Ok(Some(()));
        }
        let gethash_options = operator == "PUSHNEW"
            && items.len() > expected
            && matches!(
                &items[2].kind,
                FormKind::List(place_items)
                    if place_items.len() == 3
                        && Self::symbol_name_info(&place_items[0], "list place operator")
                            .ok()
                            .is_some_and(|(name, _)| name == "GETHASH")
            );
        if items.len() != expected && !gethash_options {
            return Err(Self::arity_error(
                items,
                &operator,
                if operator == "PUSH" { "two" } else { "one" },
                span,
            ));
        }
        let place = &items[expected - 1];
        if let FormKind::List(place_items) = &place.kind {
            if place_items.len() == 3
                && Self::symbol_name_info(&place_items[0], "list place operator")
                    .ok()
                    .is_some_and(|(name, _)| name == "GETHASH")
            {
                if operator == "PUSHNEW" && items.len() > expected {
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
                        match keyword.to_ascii_uppercase().as_str() {
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
                    self.compile_expression(function, &place_items[1])?;
                    self.compile_expression(function, &place_items[2])?;
                    self.emit(
                        function,
                        Instruction::PushNewGethashOptions {
                            test_not,
                            has_key,
                            key_before_test,
                        },
                        items[0].span,
                    )?;
                    return Ok(Some(()));
                }
                if operator == "PUSH" || operator == "PUSHNEW" {
                    self.compile_expression(function, &items[1])?;
                }
                self.compile_expression(function, &place_items[1])?;
                self.compile_expression(function, &place_items[2])?;
                self.emit(
                    function,
                    match operator.as_str() {
                        "PUSH" => Instruction::PushGethash,
                        "PUSHNEW" => Instruction::PushNewGethash,
                        _ => Instruction::PopGethash,
                    },
                    items[0].span,
                )?;
                return Ok(Some(()));
            }
        }
        let generalized = generalized_list_place(place);
        if let Some((index_form, target, name, escaped)) = nth_list_place(place) {
            if matches!(operator.as_str(), "PUSH" | "PUSHNEW") {
                self.compile_expression(function, &items[1])?;
            }
            self.compile_expression(function, index_form)?;
            self.compile_expression(function, target)?;
            self.emit(
                function,
                if operator == "PUSHNEW" {
                    Instruction::ListMutationNthPushNew { name, escaped }
                } else {
                    Instruction::ListMutationNthDynamic { operator, name, escaped }
                },
                items[0].span,
            )?;
            return Ok(Some(()));
        }
        let symbol_place = Self::symbol_name_info(place, "list place").ok();
        if generalized.is_none() && symbol_place.is_none() {
            return Ok(None);
        }
        if matches!(operator.as_str(), "PUSH" | "PUSHNEW") {
            self.compile_expression(function, &items[1])?;
        }
        if let Some((_, name, _)) = &generalized {
            self.emit(function, Instruction::Load(name.clone()), place.span)?;
        } else {
            self.compile_expression(function, place)?;
        }
        self.emit(
            function,
            if let Some((accessors, name, escaped)) = generalized {
                if accessors.len() > 1 {
                    Instruction::NestedListPlaceMutation {
                        accessors,
                        operator,
                        name,
                        escaped,
                    }
                } else {
                    Instruction::ListPlaceMutation {
                        operator,
                        accessor: accessors[0].clone(),
                        name,
                        escaped,
                    }
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

fn nth_list_place(place: &Form) -> Option<(&Form, &Form, String, bool)> {
    let FormKind::List(items) = &place.kind else { return None };
    if items.len() != 3
        || CompileState::symbol_name_info(&items[0], "list place operator")
            .ok()?
            .0
            != "NTH"
    {
        return None;
    }
    let (name, escaped) = CompileState::symbol_name_info(&items[2], "list place target").ok()?;
    Some((&items[1], &items[2], name, escaped))
}
