use crate::{CompileError, CompileState, Form, FunctionId, Span};

impl CompileState {
    pub(super) fn dispatch_core_and_control_forms(
        &mut self,
        name: &str,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Option<Result<(), CompileError>> {
        Some(match name {
            "QUOTE" => self.compile_quote(function, span, items),
            "QUASIQUOTE" => self.compile_quasiquote(function, span, items),
            "DECLARE" | "DECLAIM" | "PROCLAIM" => self.compile_declare(function, span, name, items),
            "LOCALLY" => self.compile_locally(function, items),
            "PROGN" => self.compile_progn(function, items),
            "EVAL-WHEN" => self.compile_eval_when(function, span, items),
            "WITH-COMPILATION-UNIT" => self.compile_with_compilation_unit(function, span, items),
            "LOAD-TIME-VALUE" => self.compile_load_time_value(function, span, items),
            "NTH-VALUE" => self.compile_nth_value(function, span, items),
            "PUSH" | "POP" | "PUSHNEW" | "ROTATEF" | "SHIFTF" | "REMF" => {
                self.compile_runtime_definition(function, span, items)
            }
            "PSETF" => self.compile_psetf(function, span, items),
            "GET-SETF-EXPANSION" => self.compile_get_setf_expansion(function, span, items),
            "DEFINE-SYMBOL-MACRO" => self.compile_define_symbol_macro(function, span, items),
            "DEFINE-SETF-EXPANDER" => self.compile_define_setf_expander(function, span, items),
            "DEFINE-MODIFY-MACRO" => self.compile_define_modify_macro(function, span, items),
            "DEFSETF" => self.compile_defsetf(function, span, items),
            "THE" => self.compile_the(function, span, items),
            "IF" => self.compile_if(function, span, items),
            "PROG1" => self.compile_prog1(function, span, items),
            "PROG2" => self.compile_prog2(function, span, items),
            "PROG" => self.compile_prog(function, span, items, false),
            "PROG*" => self.compile_prog(function, span, items, true),
            "VALUES" => self.compile_values(function, span, items),
            "IGNORE-ERRORS" => self.compile_ignore_errors(function, span, items),
            "HANDLER-CASE" => self.compile_handler_case(function, span, items),
            "HANDLER-BIND" => self.compile_handler_bind(function, span, items),
            "RESTART-BIND" => self.compile_restart_bind(function, span, items),
            "CATCH" => self.compile_catch(function, span, items),
            "WITH-SIMPLE-RESTART" => self.compile_with_simple_restart(function, span, items),
            "WITH-CONDITION-RESTARTS" => {
                self.compile_with_condition_restarts(function, span, items)
            }
            "WITH-OPEN-FILE" => self.compile_with_open_file(function, span, items),
            "WITH-OPEN-STREAM" => self.compile_with_open_stream(function, span, items),
            "WITH-INPUT-FROM-STRING" => self.compile_with_input_from_string(function, span, items),
            "WITH-OUTPUT-TO-STRING" => self.compile_with_output_to_string(function, span, items),
            "RESTART-CASE" => self.compile_restart_case(function, span, items),
            "PROGV" => self.compile_progv(function, span, items),
            "THROW" => self.compile_throw(function, span, items),
            "UNWIND-PROTECT" => self.compile_unwind_protect(function, span, items),
            "BLOCK" => self.compile_block(function, span, items),
            "RETURN" => self.compile_return(function, span, items),
            "RETURN-FROM" => self.compile_return_from(function, span, items),
            "TAGBODY" => self.compile_tagbody(function, span, items),
            "GO" => self.compile_go(function, span, items),
            "MULTIPLE-VALUE-BIND" => self.compile_multiple_value_bind(function, span, items),
            "MULTIPLE-VALUE-CALL" => self.compile_multiple_value_call(function, span, items),
            "MULTIPLE-VALUE-LIST" => self.compile_multiple_value_list(function, span, items),
            "MULTIPLE-VALUE-PROG1" => self.compile_multiple_value_prog1(function, span, items),
            _ => return None,
        })
    }
}
