#![allow(clippy::wildcard_imports)]
use super::super::*;
use super::setf_emit::emit_pop_if_needed;

impl CompileState {
    pub(super) fn compile_setf_aref_place(
        &mut self,
        function: FunctionId,
        place: &Form,
        value_form: &Form,
        index: usize,
        pair_count: usize,
    ) -> Result<bool, CompileError> {
        let FormKind::List(items) = &place.kind else {
            return Ok(false);
        };
        if items.len() < 3 {
            return Ok(false);
        }
        let Some((operator, _)) = Self::symbol_name_info(&items[0], "setf place operator").ok()
        else {
            return Ok(false);
        };
        if !matches!(operator.as_str(), "AREF" | "SVREF" | "ROW-MAJOR-AREF") {
            return Ok(false);
        }
        let Some((name, escaped)) = Self::symbol_name_info(&items[1], "setf aref target").ok()
        else {
            return Ok(false);
        };
        self.compile_expression(function, &items[1])?;
        for index_form in &items[2..] {
            self.compile_expression(function, index_form)?;
        }
        self.compile_expression(function, value_form)?;
        self.emit(
            function,
            Instruction::SetfArefDynamic {
                rank: items.len() - 2,
                operator,
                name,
                escaped,
            },
            place.span,
        )?;
        emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
        Ok(true)
    }
}
