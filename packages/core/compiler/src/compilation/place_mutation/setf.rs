#![allow(clippy::wildcard_imports)]
use super::super::*;
use super::setf_emit::emit_pop_if_needed;
use super::setf_validation::validate_setf_items;

impl CompileState {
    pub(crate) fn compile_setf(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let operands = validate_setf_items(items, span)?;
        let (pairs, _) = operands.as_chunks::<2>();
        let pair_count = operands.len() / 2;
        for (index, [place, value_form]) in pairs.iter().enumerate() {
            if self.compile_setf_nth_place(function, place, value_form, index, pair_count)? {
                continue;
            }
            if self.compile_setf_aref_place(function, place, value_form, index, pair_count)? {
                continue;
            }
            if self.compile_setf_bit_place(function, place, value_form, index, pair_count)? {
                continue;
            }
            if self.compile_setf_bitfield_place(function, place, value_form, index, pair_count)? {
                continue;
            }
            if self.compile_setf_element_place(function, place, value_form, index, pair_count)? {
                continue;
            }
            if self.compile_setf_subseq_place(function, place, value_form, index, pair_count)? {
                continue;
            }
            if self.compile_setf_getf_place(function, place, value_form)? {
                emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
                continue;
            }
            if self.compile_setf_symbol_plist_place(function, place, value_form)? {
                emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
                continue;
            }
            if self.compile_setf_symbol_cell_place(function, place, value_form)? {
                emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
                continue;
            }
            if self.compile_setf_property_place(function, place, value_form)? {
                emit_pop_if_needed(self, function, index, pair_count, value_form.span)?;
                continue;
            }
            self.compile_setf_fallback(function, place, value_form)?;
            if index + 1 < pair_count {
                self.emit(function, Instruction::Pop, value_form.span)?;
            }
        }
        Ok(())
    }
}
