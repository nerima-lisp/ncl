#![allow(clippy::wildcard_imports)]
use super::super::*;
use super::setf_emit::emit_pop_if_needed;

impl CompileState {
    pub(super) fn compile_setf_fill_pointer_place(
        &mut self, function: FunctionId, place: &Form, value_form: &Form,
        index: usize, pair_count: usize,
    ) -> Result<bool, CompileError> {
        let FormKind::List(items) = &place.kind else { return Ok(false) };
        if items.len() != 2 || !Self::symbol_name_info(&items[0], "setf fill-pointer operator")
            .ok().is_some_and(|(name, _)| name == "FILL-POINTER") { return Ok(false); }
        self.compile_expression(function, &items[1])?;
        self.compile_expression(function, value_form)?;
        if let Some((name, escaped)) = Self::symbol_name_info(&items[1], "setf fill-pointer target").ok() {
            self.emit(function, Instruction::SetfFillPointerDynamic { name, escaped }, place.span)?;
        } else {
            self.emit(function, Instruction::SetfFillPointerValue, place.span)?;
        }
        emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
        Ok(true)
    }

    pub(super) fn compile_setf_bitfield_place(
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
        if items.len() != 3 {
            return Ok(false);
        }
        let Some((operator, _)) = Self::symbol_name_info(&items[0], "setf bitfield place operator")
            .ok()
            .filter(|(name, _)| matches!(name.as_str(), "LDB" | "MASK-FIELD"))
        else {
            return Ok(false);
        };
        let Some((name, escaped)) = Self::symbol_name_info(&items[2], "setf bitfield target").ok()
        else {
            return Ok(false);
        };
        self.compile_expression(function, &items[1])?;
        self.compile_expression(function, &items[2])?;
        self.compile_expression(function, value_form)?;
        self.emit(
            function,
            Instruction::SetfBitfieldDynamic { operator, name, escaped },
            place.span,
        )?;
        emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
        Ok(true)
    }

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
        self.compile_expression(function, &items[1])?;
        for index_form in &items[2..] {
            self.compile_expression(function, index_form)?;
        }
        self.compile_expression(function, value_form)?;
        let instruction = if let Some((name, escaped)) =
            Self::symbol_name_info(&items[1], "setf aref target").ok()
        {
            Instruction::SetfArefDynamic {
                rank: items.len() - 2,
                operator,
                name,
                escaped,
            }
        } else {
            Instruction::SetfArefValue {
                rank: items.len() - 2,
                operator,
            }
        };
        self.emit(function, instruction, place.span)?;
        emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
        Ok(true)
    }

    pub(super) fn compile_setf_bit_place(
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
        if !Self::symbol_name_info(&items[0], "setf place operator")
            .ok()
            .is_some_and(|(name, _)| name == "BIT")
        {
            return Ok(false);
        }
        self.compile_expression(function, &items[1])?;
        for index_form in &items[2..] {
            self.compile_expression(function, index_form)?;
        }
        self.compile_expression(function, value_form)?;
        let instruction = if let Some((name, escaped)) =
            Self::symbol_name_info(&items[1], "setf bit target").ok()
        {
            Instruction::SetfBitDynamic { rank: items.len() - 2, name, escaped }
        } else {
            Instruction::SetfBitValue { rank: items.len() - 2 }
        };
        self.emit(function, instruction, place.span)?;
        emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
        Ok(true)
    }

    pub(super) fn compile_setf_element_place(
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
        if items.len() != 3 {
            return Ok(false);
        }
        let Some((operator, _)) = Self::symbol_name_info(&items[0], "setf place operator")
            .ok()
            .filter(|(name, _)| matches!(name.as_str(), "ELT" | "CHAR" | "SCHAR"))
        else {
            return Ok(false);
        };
        self.compile_expression(function, &items[1])?;
        self.compile_expression(function, &items[2])?;
        self.compile_expression(function, value_form)?;
        let instruction = if let Some((name, escaped)) =
            Self::symbol_name_info(&items[1], "setf element target").ok()
        {
            Instruction::SetfElementDynamic { operator, name, escaped }
        } else {
            Instruction::SetfElementValue { operator }
        };
        self.emit(function, instruction, place.span)?;
        emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
        Ok(true)
    }

    pub(super) fn compile_setf_subseq_place(
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
        if !(items.len() == 3 || items.len() == 4) {
            return Ok(false);
        }
        let Some((_, _)) = Self::symbol_name_info(&items[0], "setf place operator")
            .ok()
            .filter(|(name, _)| name == "SUBSEQ")
        else {
            return Ok(false);
        };
        let Some((name, escaped)) = Self::symbol_name_info(&items[1], "setf subseq target").ok()
        else {
            return Ok(false);
        };
        self.compile_expression(function, &items[1])?;
        for bound in &items[2..] {
            self.compile_expression(function, bound)?;
        }
        self.compile_expression(function, value_form)?;
        self.emit(
            function,
            Instruction::SetfSubseqDynamic {
                has_end: items.len() == 4,
                name,
                escaped,
            },
            place.span,
        )?;
        emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
        Ok(true)
    }
}
