use crate::{CompileError, CompileState, Form, FormKind, FunctionId, Span};

impl CompileState {
    pub(super) fn dispatch_logic_and_binding_forms(
        &mut self,
        name: &str,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Option<Result<(), CompileError>> {
        Some(match name {
            "AND" => self.compile_and(function, span, items),
            "OR" => self.compile_or(function, span, items),
            "WHEN" => self.compile_when(function, span, items, true),
            "UNLESS" => self.compile_when(function, span, items, false),
            "COND" => self.compile_cond(function, span, items),
            "CASE" | "ECASE" => self.compile_case(function, span, items),
            "TYPECASE" | "ETYPECASE" => self.compile_typecase(function, span, items),
            "LAMBDA" => self.compile_lambda(function, span, items),
            "FUNCTION" => self.compile_function(function, span, items),
            "DEFINE" => self.compile_define(function, span, items),
            "DEFUN" => self.compile_defun(function, span, items),
            "SETQ" => self.compile_setq(function, span, items),
            "PSETQ" => self.compile_psetq(function, span, items),
            "MULTIPLE-VALUE-SETQ" => self.compile_multiple_value_setq(function, span, items),
            "SETF" => self.compile_setf(function, span, items),
            "INCF" => {
                if matches!(
                    items.get(1).map(|place| &place.kind),
                    Some(FormKind::Atom(_))
                ) {
                    self.compile_modify_symbol(function, span, items, "INCF", "+")
                } else {
                    self.compile_runtime_definition(function, span, items)
                }
            }
            "DECF" => {
                if matches!(
                    items.get(1).map(|place| &place.kind),
                    Some(FormKind::Atom(_))
                ) {
                    self.compile_modify_symbol(function, span, items, "DECF", "-")
                } else {
                    self.compile_runtime_definition(function, span, items)
                }
            }
            "DEFVAR" => self.compile_defvar(function, span, items, false),
            "DEFPARAMETER" => self.compile_defvar(function, span, items, true),
            "DEFCONSTANT" => self.compile_runtime_definition(function, span, items),
            "DEFSTRUCT" => self.compile_defstruct(function, span, items),
            "EVAL" => self.compile_eval(function, span, items),
            "FUNCALL" => self.compile_funcall(function, span, items),
            "APPLY" => self.compile_apply(function, span, items),
            "MAP-INTO" => self.compile_map_into(function, span, items),
            "MAPCAR" => self.compile_mapcar(function, span, items),
            "DESTRUCTURING-BIND" => self.compile_destructuring_bind(function, span, items),
            "LET" => self.compile_let(function, span, items, false),
            "LET*" => self.compile_let(function, span, items, true),
            "FLET" => self.compile_flet(function, span, items, false),
            "LABELS" => self.compile_flet(function, span, items, true),
            "DOTIMES" => self.compile_dotimes(function, span, items),
            "DOLIST" => self.compile_dolist(function, span, items),
            "DO" => self.compile_do(function, span, items, false),
            "DO*" => self.compile_do(function, span, items, true),
            _ => return None,
        })
    }
}
