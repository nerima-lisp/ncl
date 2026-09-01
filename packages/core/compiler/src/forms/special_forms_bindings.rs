use crate::{CompileError, CompileState, Form, FunctionId, Span};

impl CompileState {
    pub(super) fn dispatch_logic_and_binding_forms(
        &mut self,
        name: &str,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Option<Result<(), CompileError>> {
        if matches!(
            name,
            "FIND"
                | "POSITION"
                | "COUNT"
                | "FIND-IF"
                | "POSITION-IF"
                | "COUNT-IF"
                | "FIND-IF-NOT"
                | "POSITION-IF-NOT"
                | "COUNT-IF-NOT"
                | "SEARCH"
                | "MISMATCH"
                | "MEMBER"
                | "MEMBER-IF"
                | "MEMBER-IF-NOT"
                | "ADJOIN"
                | "ASSOC"
                | "ASSOC-IF"
                | "ASSOC-IF-NOT"
                | "RASSOC"
                | "RASSOC-IF"
                | "RASSOC-IF-NOT"
                | "REMOVE"
                | "REMOVE-IF"
                | "REMOVE-IF-NOT"
                | "DELETE"
                | "DELETE-IF"
                | "DELETE-IF-NOT"
                | "REMOVE-DUPLICATES"
                | "DELETE-DUPLICATES"
                | "SUBSTITUTE"
                | "SUBSTITUTE-IF"
                | "SUBSTITUTE-IF-NOT"
                | "NSUBSTITUTE"
                | "NSUBSTITUTE-IF"
                | "NSUBSTITUTE-IF-NOT"
        ) && self.has_local_function(name)
        {
            return None;
        }
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
            "INCF" => self.compile_modify(function, span, items, "INCF", "+"),
            "DECF" => self.compile_modify(function, span, items, "DECF", "-"),
            "DEFVAR" => self.compile_defvar(function, span, items, false),
            "DEFPARAMETER" => self.compile_defvar(function, span, items, true),
            "DEFCONSTANT" => self.compile_defconstant(function, span, items),
            "DEFSTRUCT" => self.compile_defstruct(function, span, items),
            "DEFCLASS" => self.compile_defclass(function, span, items),
            "DEFGENERIC" => self.compile_defgeneric(function, span, items),
            "DEFMETHOD" => self.compile_defmethod(function, span, items),
            "EVAL" => self.compile_eval(function, span, items),
            "FUNCALL" => self.compile_funcall(function, span, items),
            "APPLY" => self.compile_apply(function, span, items),
            "MAP-INTO" => self.compile_map_into(function, span, items),
            "EVERY" | "SOME" | "NOTANY" | "NOTEVERY" => {
                self.compile_sequence_quantifier(function, span, items, name)
            }
            "MAP" => self.compile_sequence_mapping(function, span, items),
            "REDUCE" => self.compile_sequence_reduce(function, span, items),
            "MERGE" => self.compile_sequence_merge(function, span, items),
            "SORT" | "STABLE-SORT" => self.compile_sequence_sort(function, span, items, name),
            "FIND" | "POSITION" | "COUNT" | "FIND-IF" | "POSITION-IF" | "COUNT-IF"
            | "FIND-IF-NOT" | "POSITION-IF-NOT" | "COUNT-IF-NOT" => {
                self.compile_sequence_search(function, span, items, name)
            }
            "SEARCH" | "MISMATCH" => {
                self.compile_sequence_pair_search(function, span, items, name)
            }
            "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" | "ADJOIN" => {
                self.compile_list_membership(function, span, items, name)
            }
            "ASSOC" | "ASSOC-IF" | "ASSOC-IF-NOT" | "RASSOC" | "RASSOC-IF"
            | "RASSOC-IF-NOT" => self.compile_association_search(function, span, items, name),
            "REMOVE" | "REMOVE-IF" | "REMOVE-IF-NOT" | "DELETE" | "DELETE-IF"
            | "DELETE-IF-NOT" | "REMOVE-DUPLICATES" | "DELETE-DUPLICATES" => {
                self.compile_sequence_removal(function, span, items, name)
            }
            "SUBSTITUTE" | "SUBSTITUTE-IF" | "SUBSTITUTE-IF-NOT" | "NSUBSTITUTE"
            | "NSUBSTITUTE-IF" | "NSUBSTITUTE-IF-NOT" => {
                self.compile_sequence_substitution(function, span, items, name)
            }
            "COPY-TREE" | "REVERSE" | "NREVERSE" => {
                self.compile_sequence_unary(function, span, items, name)
            }
            "CAR" | "CDR" | "FIRST" | "REST" => {
                self.compile_list_unary(function, span, items, name)
            }
            "TREE-EQUAL" => self.compile_tree_equal(function, span, items),
            "LENGTH" => self.compile_sequence_length(function, span, items),
            "ELT" => self.compile_sequence_element(function, span, items),
            "UNION" | "NUNION" | "INTERSECTION" | "NINTERSECTION" | "SET-DIFFERENCE"
            | "NSET-DIFFERENCE" | "SET-EXCLUSIVE-OR" | "NSET-EXCLUSIVE-OR" | "SUBSETP" => {
                self.compile_list_set(function, span, items, name)
            }
            "MAPCAR" | "MAPC" | "MAPL" | "MAPLIST" | "MAPCAN" | "MAPCON" => {
                self.compile_list_mapping(function, span, items, name)
            }
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
