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
                | "CAR"
                | "CDR"
                | "FIRST"
                | "REST"
                | "COPY-LIST"
                | "COPY-ALIST"
                | "ENDP"
                | "CHARACTER"
                | "CHAR"
                | "SCHAR"
                | "AREF"
                | "SVREF"
                | "BIT"
                | "ROW-MAJOR-AREF"
                | "ARRAY-ELEMENT-TYPE"
                | "ARRAY-RANK"
                | "ARRAY-DIMENSIONS"
                | "ARRAY-DIMENSION"
                | "ARRAY-TOTAL-SIZE"
                | "ARRAY-ROW-MAJOR-INDEX"
                | "ARRAY-IN-BOUNDS-P"
                | "SUBSEQ"
                | "FILL"
                | "REPLACE"
                | "CONCATENATE"
                | "MAKE-SEQUENCE"
                | "COERCE"
                | "CHAR-CODE"
                | "CHAR-INT"
                | "CODE-CHAR"
                | "INT-CHAR"
                | "LAST"
                | "BUTLAST"
                | "NBUTLAST"
                | "NTHCDR"
                | "NTH"
                | "ATOM"
                | "CONSP"
                | "LISTP"
                | "NUMBERP"
                | "COMPLEXP"
                | "INTEGERP"
                | "FLOATP"
                | "RATIONALP"
                | "STRINGP"
                | "SIMPLE-STRING-P"
                | "CHARACTERP"
                | "SYMBOLP"
                | "PACKAGEP"
                | "KEYWORDP"
                | "VECTORP"
                | "FUNCTIONP"
                | "SIMPLE-VECTOR-P"
                | "BIT-VECTOR-P"
                | "SIMPLE-BIT-VECTOR-P"
                | "ARRAYP"
                | "SIMPLE-ARRAY-P"
                | "HASH-TABLE-P"
                | "RANDOM-STATE-P"
                | "ALPHA-CHAR-P"
                | "ALPHANUMERICP"
                | "GRAPHIC-CHAR-P"
                | "STANDARD-CHAR-P"
                | "UPPER-CASE-P"
                | "LOWER-CASE-P"
                | "BOTH-CASE-P"
                | "STREAMP"
                | "INPUT-STREAM-P"
                | "OUTPUT-STREAM-P"
                | "NOT"
                | "NULL"
                | "VECTOR"
                | "LIST"
                | "LIST*"
                | "APPEND"
                | "NCONC"
                | "REVAPPEND"
                | "NRECONC"
                | "GETF"
                | "GET-PROPERTIES"
                | "GET"
                | "PUTPROP"
                | "REMPROP"
                | "SYMBOL-PLIST"
                | "BOUNDP"
                | "CONSTANTP"
                | "SYMBOL-VALUE"
                | "SET"
                | "MAKUNBOUND"
                | "FMAKUNBOUND"
                | "FBOUNDP"
                | "MACRO-FUNCTION"
                | "SPECIAL-OPERATOR-P"
                | "COMPILED-FUNCTION-P"
                | "FDEFINITION"
                | "SYMBOL-FUNCTION"
                | "FIND-PACKAGE"
                | "PACKAGE-NAME"
                | "PACKAGE-USE-LIST"
                | "PACKAGE-NICKNAMES"
                | "PACKAGE-SHADOWING-SYMBOLS"
                | "PACKAGE-USED-BY-LIST"
                | "DOCUMENTATION" | "LIST-ALL-PACKAGES"
                | "MAKE-SYMBOL" | "GENSYM" | "INTERN" | "FIND-SYMBOL"
                | "SUBTYPEP" | "CLASS-OF" | "FIND-CLASS" | "CLASS-NAME"
                | "SLOT-VALUE" | "SLOT-EXISTS-P" | "SLOT-BOUNDP" | "SLOT-MAKUNBOUND"
                | "ERROR" | "SIGNAL" | "WARN" | "CERROR" | "MAKE-CONDITION"
                | "COMPUTE-RESTARTS" | "FIND-RESTART" | "INVOKE-RESTART" | "RESTART-NAME"
                | "CALL-NEXT-METHOD" | "NEXT-METHOD-P"
                | "MAKE-INSTANCE"
                | "COMPILE" | "LOAD"
                | "USE-PACKAGE" | "UNUSE-PACKAGE" | "EXPORT" | "UNEXPORT"
                | "IMPORT" | "SHADOWING-IMPORT" | "SHADOW" | "UNINTERN"
                | "GETHASH"
                | "REMHASH"
                | "MAKE-HASH-TABLE"
                | "CLRHASH"
                | "HASH-TABLE-COUNT"
                | "HASH-TABLE-TEST"
                | "NCL-HASH-TABLE-KEYS"
                | "NCL-HASH-TABLE-VALUES"
                | "MAKE-ARRAY" | "ADJUST-ARRAY"
                | "COPY-SEQ"
                | "STRING-UPCASE" | "STRING-DOWNCASE" | "STRING-CAPITALIZE"
                | "NSTRING-UPCASE" | "NSTRING-DOWNCASE" | "NSTRING-CAPITALIZE"
                | "STRING=" | "STRING-EQUAL" | "STRING<" | "STRING>" | "STRING<=" | "STRING>="
                | "STRING-TRIM" | "STRING-LEFT-TRIM" | "STRING-RIGHT-TRIM"
                | "STRING" | "MAKE-STRING"
                | "CHAR=" | "CHAR/=" | "CHAR-EQUAL" | "CHAR-NOT-EQUAL" | "CHAR<" | "CHAR>"
                | "CHAR<=" | "CHAR>=" | "CHAR-LESSP" | "CHAR-GREATERP" | "CHAR-NOT-LESSP"
                | "CHAR-NOT-GREATERP"
                | "CHAR-UPCASE" | "CHAR-DOWNCASE" | "CHAR-NAME" | "NAME-CHAR"
                | "DIGIT-CHAR-P" | "SYMBOL-NAME" | "SYMBOL-PACKAGE"
        ) && self.has_local_function(name)
        {
            return None;
        }
        Some(match name {
            "EQL" | "EQUAL" | "EQUALP" => {
                self.compile_equality(function, span, items, name)
            }
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
            "DOCUMENTATION" | "LIST-ALL-PACKAGES" => {
                self.compile_package_listing(function, span, items, name)
            }
            "MAKE-SYMBOL" | "GENSYM" | "INTERN" | "FIND-SYMBOL" => {
                self.compile_symbol_creation(function, span, items, name)
            }
            "SUBTYPEP" | "CLASS-OF" | "FIND-CLASS" | "CLASS-NAME" => {
                self.compile_class_introspection(function, span, items, name)
            }
            "SLOT-VALUE" | "SLOT-EXISTS-P" | "SLOT-BOUNDP" | "SLOT-MAKUNBOUND" => {
                self.compile_slot_operation(function, span, items, name)
            }
            "ERROR" | "SIGNAL" | "WARN" | "CERROR" | "MAKE-CONDITION" => {
                self.compile_condition_operation(function, span, items, name)
            }
            "COMPUTE-RESTARTS" | "FIND-RESTART" | "INVOKE-RESTART" | "RESTART-NAME" => {
                self.compile_restart_operation(function, span, items, name)
            }
            "CALL-NEXT-METHOD" | "NEXT-METHOD-P" => {
                self.compile_method_operation(function, span, items, name)
            }
            "MAKE-INSTANCE" => self.compile_evaluation_operation(function, span, items, name),
            "COMPILE" | "LOAD" => self.compile_evaluation_operation(function, span, items, name),
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
            "COPY-TREE" | "COPY-SEQ" | "REVERSE" | "NREVERSE" => {
                self.compile_sequence_unary(function, span, items, name)
            }
            "CAR" | "CDR" | "FIRST" | "REST" | "COPY-LIST" | "COPY-ALIST" | "ENDP"
            | "LIST-LENGTH" | "VALUES-LIST" | "SECOND" | "THIRD" | "FOURTH" | "FIFTH" | "SIXTH"
            | "SEVENTH" | "EIGHTH" | "NINTH" | "TENTH" => {
                self.compile_list_unary(function, span, items, name)
            }
            "CHARACTER" | "CHAR-CODE" | "CHAR-INT" | "CODE-CHAR" | "INT-CHAR"
            | "CHAR-UPCASE" | "CHAR-DOWNCASE" | "CHAR-NAME" | "NAME-CHAR" => {
                self.compile_character_unary(function, span, items, name)
            }
            "SYMBOL-NAME" | "SYMBOL-PACKAGE" => self.compile_symbol_unary(function, span, items, name),
            "1+" | "1-" | "ABS" | "SIGNUM" | "ZEROP" | "PLUSP" | "MINUSP" | "EVENP"
            | "ODDP" | "LOGNOT" | "LOGCOUNT" | "INTEGER-LENGTH" | "ISQRT" | "SQRT" | "SIN" | "COS"
            | "CIS" | "TAN" | "EXP" | "ASIN" | "ACOS" | "SINH" | "COSH" | "TANH"
            | "REALPART" | "IMAGPART" | "CONJUGATE" | "PHASE" | "RATIONAL" | "RATIONALIZE"
            | "NUMERATOR" | "DENOMINATOR" => {
                self.compile_numeric_unary(function, span, items, name)
            }
            "FLOOR" | "CEILING" | "TRUNCATE" | "ROUND" => {
                self.compile_numeric_rounding(function, span, items, name)
            }
            "=" | "/=" | "<" | ">" | "<=" | ">=" => {
                self.compile_numeric_comparison(function, span, items, name)
            }
            "MIN" | "MAX" | "GCD" | "LCM" | "LOGAND" | "LOGIOR" | "LOGXOR" => {
                self.compile_numeric_fold(function, span, items, name)
            }
            "MOD" | "REM" | "ASH" | "LOGTEST" | "LOGANDC1" | "LOGANDC2"
            | "LOGEQV" | "LOGNAND" | "LOGNOR" | "LOGORC1" | "LOGORC2" | "EXPT" => {
                self.compile_numeric_binary(function, span, items, name)
            }
            "LOGBITP" => self.compile_numeric_binary(function, span, items, name),
            "BOOLE" => self.compile_numeric_boole(function, span, items),
            "BYTE" | "LDB" | "MASK-FIELD" | "DPB" | "DEPOSIT-FIELD" => {
                self.compile_numeric_bitfield(function, span, items, name)
            }
            "FLOAT" | "FLOAT-SIGN" | "FLOAT-DIGITS" | "FLOAT-PRECISION" | "FLOAT-RADIX"
            | "SCALE-FLOAT" | "DECODE-FLOAT" | "INTEGER-DECODE-FLOAT" | "LOG" | "ATAN" | "COMPLEX" => {
                self.compile_numeric_float(function, span, items, name)
            }
            "LAST" | "BUTLAST" | "NBUTLAST" => {
                self.compile_list_tail(function, span, items, name)
            }
            "NTH" | "NTHCDR" => self.compile_list_binary(function, span, items, name),
            "ATOM" | "CONSP" | "LISTP" | "NUMBERP" | "COMPLEXP" | "INTEGERP"
            | "FLOATP" | "RATIONALP" | "STRINGP" | "SIMPLE-STRING-P" | "CHARACTERP"
            | "SYMBOLP" | "PACKAGEP" | "KEYWORDP" | "VECTORP" | "FUNCTIONP"
            | "SIMPLE-VECTOR-P" | "BIT-VECTOR-P" | "SIMPLE-BIT-VECTOR-P" | "ARRAYP"
            | "SIMPLE-ARRAY-P" | "HASH-TABLE-P" | "RANDOM-STATE-P" | "ALPHA-CHAR-P"
            | "ALPHANUMERICP" | "GRAPHIC-CHAR-P" | "STANDARD-CHAR-P" | "UPPER-CASE-P"
            | "LOWER-CASE-P" | "BOTH-CASE-P" | "DIGIT-CHAR-P" | "STREAMP" | "INPUT-STREAM-P"
            | "NOT" | "NULL"
            | "OUTPUT-STREAM-P" => {
                if name == "DIGIT-CHAR-P" {
                    self.compile_character_digit_predicate(function, span, items)
                } else {
                    self.compile_type_predicate(function, span, items, name)
                }
            }
            "TREE-EQUAL" => self.compile_tree_equal(function, span, items),
            "LENGTH" => self.compile_sequence_length(function, span, items),
            "ELT" => self.compile_sequence_element(function, span, items),
            "SUBSEQ" => self.compile_sequence_subseq(function, span, items),
            "FILL" | "REPLACE" => self.compile_sequence_mutation(function, span, items, name),
            "CONCATENATE" => self.compile_sequence_concatenate(function, span, items),
            "MAKE-SEQUENCE" | "COERCE" => {
                self.compile_sequence_conversion(function, span, items, name)
            }
            "STRING-UPCASE" | "STRING-DOWNCASE" | "STRING-CAPITALIZE"
            | "NSTRING-UPCASE" | "NSTRING-DOWNCASE" | "NSTRING-CAPITALIZE" => {
                self.compile_string_case(function, span, items, name)
            }
            "STRING=" | "STRING-EQUAL" | "STRING<" | "STRING>" | "STRING<=" | "STRING>=" => {
                self.compile_string_comparison(function, span, items, name)
            }
            "STRING-TRIM" | "STRING-LEFT-TRIM" | "STRING-RIGHT-TRIM" => {
                self.compile_string_trim(function, span, items, name)
            }
            "STRING" | "MAKE-STRING" => {
                self.compile_string_construction(function, span, items, name)
            }
            "VECTOR" => self.compile_vector(function, span, items),
            "LIST" | "LIST*" => self.compile_list_construction(function, span, items, name),
            "APPEND" | "NCONC" | "REVAPPEND" | "NRECONC" | "ACONS" | "PAIRLIS" => {
                self.compile_list_append(function, span, items, name)
            }
            "GETF" | "GET-PROPERTIES" | "GET" | "PUTPROP" | "REMPROP" | "SYMBOL-PLIST" => {
                self.compile_property_list(function, span, items, name)
            }
            "BOUNDP" | "CONSTANTP" | "SYMBOL-VALUE" => {
                self.compile_symbol_value(function, span, items, name)
            }
            "SET" | "MAKUNBOUND" | "FMAKUNBOUND" => {
                self.compile_symbol_binding(function, span, items, name)
            }
            "FBOUNDP" | "MACRO-FUNCTION" | "SPECIAL-OPERATOR-P" | "COMPILED-FUNCTION-P"
            | "FDEFINITION" | "SYMBOL-FUNCTION" => {
                self.compile_symbol_function(function, span, items, name)
            }
            "FIND-ALL-SYMBOLS" | "FIND-PACKAGE" | "PACKAGE-NAME" | "PACKAGE-USE-LIST" | "PACKAGE-NICKNAMES"
            | "PACKAGE-SHADOWING-SYMBOLS" | "PACKAGE-USED-BY-LIST" => {
                self.compile_package_introspection(function, span, items, name)
            }
            "USE-PACKAGE" | "UNUSE-PACKAGE" | "EXPORT" | "UNEXPORT"
            | "IMPORT" | "SHADOWING-IMPORT" | "SHADOW" | "UNINTERN" => {
                self.compile_package_mutation(function, span, items, name)
            }
            "GETHASH" | "REMHASH" | "MAKE-HASH-TABLE" | "CLRHASH" | "HASH-TABLE-COUNT"
            | "HASH-TABLE-TEST" | "NCL-HASH-TABLE-KEYS" | "NCL-HASH-TABLE-VALUES" => {
                self.compile_hash_table(function, span, items, name)
            }
            "MAKE-ARRAY" => self.compile_array_construction(function, span, items),
            "ADJUST-ARRAY" => self.compile_array_adjustment(function, span, items),
            "MAKE-LIST" => self.compile_list_construction_with_options(function, span, items),
            "CHAR=" | "CHAR/=" | "CHAR-EQUAL" | "CHAR-NOT-EQUAL" | "CHAR<" | "CHAR>"
            | "CHAR<=" | "CHAR>=" | "CHAR-LESSP" | "CHAR-GREATERP" | "CHAR-NOT-LESSP"
            | "CHAR-NOT-GREATERP" => self.compile_character_comparison(function, span, items, name),
            "CHAR" | "SCHAR" => self.compile_character_element(function, span, items, name),
            "AREF" | "BIT" => self.compile_array_element(function, span, items, name, false),
            "SVREF" | "ROW-MAJOR-AREF" => {
                self.compile_array_element(function, span, items, name, true)
            }
            "ARRAY-ROW-MAJOR-INDEX" | "ARRAY-IN-BOUNDS-P" => {
                self.compile_array_element(function, span, items, name, false)
            }
            "ARRAY-ELEMENT-TYPE" | "ARRAY-RANK" | "ARRAY-DIMENSIONS" | "ARRAY-TOTAL-SIZE" => {
                self.compile_array_metadata(function, span, items, name, 1)
            }
            "ARRAY-DIMENSION" => self.compile_array_metadata(function, span, items, name, 2),
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
