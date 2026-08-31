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
        if let Some(places) = places {
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
            return Ok(Some(()));
        }
        let nested = place_forms
            .iter()
            .map(crate::generalized_list_place)
            .collect::<Option<Vec<_>>>();
        let Some(nested) = nested.filter(|places| !places.is_empty()) else {
            return Ok(None);
        };
        for place in place_forms {
            let (_, name, escaped) = crate::generalized_list_place(place).expect("checked above");
            self.emit(
                function,
                if escaped {
                    Instruction::LoadExact(name)
                } else {
                    Instruction::Load(name)
                },
                place.span,
            )?;
        }
        if operator == "SHIFTF" {
            self.compile_expression(function, &items[items.len() - 1])?;
        }
        self.emit(
            function,
            if operator == "ROTATEF" {
                Instruction::RotatefNestedList(nested)
            } else {
                Instruction::ShiftfNestedList(nested)
            },
            items[0].span,
        )?;
        Ok(Some(()))
    }
}
