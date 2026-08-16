impl Runtime {
    fn apply_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match name {
            "ERROR" | "SIGNAL" | "WARN" | "CERROR" | "MAKE-CONDITION" => {
                self.apply_condition_primitive(name, arguments, environment, span)
            }
            "EVAL" | "COMPILE" | "LOAD" => {
                self.apply_evaluation_primitive(name, arguments, environment, span)
            }
            "MAKE-INSTANCE"
            | "ALLOCATE-INSTANCE"
            | "CHANGE-CLASS"
            | "REINITIALIZE-INSTANCE"
            | "SHARED-INITIALIZE"
            | "ENSURE-GENERIC-FUNCTION"
            | "FIND-METHOD"
            | "COMPUTE-APPLICABLE-METHODS"
            | "GENERIC-FUNCTION-METHODS"
            | "GENERIC-FUNCTION-CLASS"
            | "GENERIC-FUNCTION-NAME"
            | "METHOD-CLASS"
            | "METHOD-COMBINATION"
            | "METHOD-FUNCTION"
            | "METHOD-GENERIC-FUNCTION"
            | "METHOD-LAMBDA-LIST"
            | "METHOD-QUALIFIERS"
            | "METHOD-SPECIALIZERS"
            | "SLOT-VALUE"
            | "SUBTYPEP"
            | "UPGRADED-ARRAY-ELEMENT-TYPE"
            | "CLASS-OF"
            | "FIND-CLASS"
            | "CLASS-NAME"
            | "SLOT-EXISTS-P"
            | "SLOT-BOUNDP"
            | "SLOT-MAKUNBOUND"
            | "CALL-NEXT-METHOD"
            | "NEXT-METHOD-P" => {
                self.apply_clos_primitive(name, arguments, environment, span)
            }
            "MAKE-SYMBOL" | "GENSYM" => {
                self.apply_symbol_primitive(name, arguments, environment, span)
            }
            "MAKE-PACKAGE"
            | "INTERN"
            | "FIND-SYMBOL"
            | "FIND-PACKAGE"
            | "DELETE-PACKAGE"
            | "RENAME-PACKAGE"
            | "PACKAGE-NAME"
            | "PACKAGE-USE-LIST"
            | "PACKAGE-NICKNAMES"
            | "PACKAGE-SHADOWING-SYMBOLS"
            | "PACKAGE-USED-BY-LIST"
            | "DOCUMENTATION"
            | "LIST-ALL-PACKAGES"
            | "USE-PACKAGE"
            | "UNUSE-PACKAGE"
            | "EXPORT"
            | "UNEXPORT"
            | "IMPORT"
            | "SHADOWING-IMPORT"
            | "SHADOW"
            | "UNINTERN" => {
                self.apply_package_primitive(name, arguments, environment, span)
            }
            "BOUNDP"
            | "CONSTANTP"
            | "FBOUNDP"
            | "MACRO-FUNCTION"
            | "COMPILER-MACRO-FUNCTION"
            | "SPECIAL-OPERATOR-P"
            | "COMPILED-FUNCTION-P"
            | "FUNCTION-LAMBDA-EXPRESSION"
            | "FDEFINITION"
            | "SYMBOL-FUNCTION"
            | "SYMBOL-VALUE"
            | "GET"
            | "PUTPROP"
            | "REMPROP"
            | "SYMBOL-PLIST"
            | "SET"
            | "MAKUNBOUND"
            | "FMAKUNBOUND" => {
                self.apply_symbol_binding_primitive(name, arguments, environment, span)
            }
            "COMPUTE-RESTARTS" | "FIND-RESTART" | "RESTART-NAME" | "INVOKE-RESTART" => {
                self.apply_restart_primitive(name, arguments, environment, span)
            }
            "MAP"
            | "REDUCE"
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
            | "UNION"
            | "NUNION"
            | "INTERSECTION"
            | "NINTERSECTION"
            | "SET-DIFFERENCE"
            | "NSET-DIFFERENCE"
            | "SET-EXCLUSIVE-OR"
            | "NSET-EXCLUSIVE-OR"
            | "SUBSETP"
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
            | "FIND"
            | "POSITION"
            | "COUNT"
            | "SEARCH"
            | "MISMATCH"
            | "SORT"
            | "STABLE-SORT"
            | "MERGE"
            | "EVERY"
            | "SOME"
            | "NOTANY"
            | "NOTEVERY"
            | "MAP-INTO"
            | "MAPCAR"
            | "MAPC"
            | "MAPL"
            | "MAPLIST"
            | "MAPCAN"
            | "MAPCON" => {
                self.apply_sequence_primitive(name, arguments, environment, span)
            }
            _ => Err(self.invalid("unknown runtime primitive", span)),
        }
    }
}

include!("primitives/conditions.rs");
include!("primitives/evaluation.rs");
include!("primitives/clos.rs");
include!("primitives/symbols.rs");
include!("primitives/packages.rs");
include!("primitives/symbol_bindings.rs");
include!("primitives/restarts.rs");
include!("primitives/sequences.rs");
