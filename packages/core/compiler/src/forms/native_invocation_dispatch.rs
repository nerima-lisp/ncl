use crate::{CompileError, CompileState, Form, FunctionId, Span};

impl CompileState {
    pub(super) fn dispatch_native_invocation(
        &mut self,
        name: &str,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Option<Result<(), CompileError>> {
        if self.has_local_function(name) {
            return None;
        }

        let result = match name {
            "MAPCAR" | "MAPC" | "MAPL" | "MAPLIST" | "MAPCAN" | "MAPCON" => {
                self.compile_list_mapping(function, span, items, name)
            }
            "EVERY" | "SOME" | "NOTANY" | "NOTEVERY" => {
                self.compile_sequence_quantifier(function, span, items, name)
            }
            "MAP" => self.compile_sequence_mapping(function, span, items),
            "REDUCE" => self.compile_sequence_reduce(function, span, items),
            "MERGE" => self.compile_sequence_merge(function, span, items),
            "SORT" | "STABLE-SORT" => self.compile_sequence_sort(function, span, items, name),
            "LENGTH" => self.compile_sequence_length(function, span, items),
            "ELT" => self.compile_sequence_element(function, span, items),
            "SUBSEQ" => self.compile_sequence_subseq(function, span, items),
            "FILL" | "REPLACE" => self.compile_sequence_mutation(function, span, items, name),
            "CONCATENATE" => self.compile_sequence_concatenate(function, span, items),
            "MAKE-SEQUENCE" | "COERCE" => {
                self.compile_sequence_conversion(function, span, items, name)
            }
            "MAKE-HASH-TABLE"
            | "GETHASH"
            | "REMHASH"
            | "CLRHASH"
            | "HASH-TABLE-COUNT"
            | "HASH-TABLE-SIZE"
            | "HASH-TABLE-TEST"
            | "HASH-TABLE-REHASH-SIZE"
            | "HASH-TABLE-REHASH-THRESHOLD"
            | "NCL-HASH-TABLE-KEYS"
            | "NCL-HASH-TABLE-VALUES"
            | "MAPHASH" => self.compile_hash_table(function, span, items, name),
            "AREF" | "BIT" | "ARRAY-ROW-MAJOR-INDEX" | "ARRAY-IN-BOUNDS-P" => {
                self.compile_array_element(function, span, items, name, false)
            }
            "SVREF" | "ROW-MAJOR-AREF" => {
                self.compile_array_element(function, span, items, name, true)
            }
            "ARRAY-ELEMENT-TYPE" | "ARRAY-RANK" | "ARRAY-DIMENSIONS" | "ARRAY-TOTAL-SIZE" => {
                self.compile_array_metadata(function, span, items, name, 1)
            }
            "ARRAY-DIMENSION" => self.compile_array_metadata(function, span, items, name, 2),
            "MAKE-ARRAY" => self.compile_array_construction(function, span, items),
            "ADJUST-ARRAY" => self.compile_array_adjustment(function, span, items),
            "VECTOR" => self.compile_vector(function, span, items),
            "FILL-POINTER" | "VECTOR-POP" | "VECTOR-PUSH" | "VECTOR-PUSH-EXTEND" => {
                self.compile_vector_operation(function, span, items, name)
            }
            "FIND" | "POSITION" | "COUNT" | "FIND-IF" | "POSITION-IF" | "COUNT-IF"
            | "FIND-IF-NOT" | "POSITION-IF-NOT" | "COUNT-IF-NOT" => {
                self.compile_sequence_search(function, span, items, name)
            }
            "SEARCH" | "MISMATCH" => self.compile_sequence_pair_search(function, span, items, name),
            "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" | "ADJOIN" => {
                self.compile_list_membership(function, span, items, name)
            }
            "ASSOC" | "ASSOC-IF" | "ASSOC-IF-NOT" | "RASSOC" | "RASSOC-IF" | "RASSOC-IF-NOT" => {
                self.compile_association_search(function, span, items, name)
            }
            "REMOVE" | "REMOVE-IF" | "REMOVE-IF-NOT" | "DELETE" | "DELETE-IF" | "DELETE-IF-NOT"
            | "DELETE-DUPLICATES" | "REMOVE-DUPLICATES" => {
                self.compile_sequence_removal(function, span, items, name)
            }
            "SUBSTITUTE" | "SUBSTITUTE-IF" | "SUBSTITUTE-IF-NOT" | "NSUBSTITUTE"
            | "NSUBSTITUTE-IF" | "NSUBSTITUTE-IF-NOT" => {
                self.compile_sequence_substitution(function, span, items, name)
            }
            "COPY-TREE" | "COPY-SEQ" | "REVERSE" | "NREVERSE" => {
                self.compile_sequence_unary(function, span, items, name)
            }
            "EQ" | "EQL" | "EQUAL" | "EQUALP" => self.compile_equality(function, span, items, name),
            "ATOM"
            | "CONSP"
            | "LISTP"
            | "NUMBERP"
            | "INTEGERP"
            | "STRINGP"
            | "CHARACTERP"
            | "SYMBOLP"
            | "VECTORP"
            | "FUNCTIONP"
            | "SIMPLE-VECTOR-P"
            | "BIT-VECTOR-P"
            | "SIMPLE-BIT-VECTOR-P"
            | "ARRAYP"
            | "SIMPLE-ARRAY-P"
            | "HASH-TABLE-P"
            | "RANDOM-STATE-P"
            | "STREAMP"
            | "INPUT-STREAM-P"
            | "OUTPUT-STREAM-P"
            | "OPEN-STREAM-P" => self.compile_type_predicate(function, span, items, name),
            "TYPEP" => self.compile_typep(function, span, items),
            "1+" | "1-" | "ABS" | "SIGNUM" | "ZEROP" | "PLUSP" | "MINUSP" | "EVENP" | "ODDP"
            | "LOGNOT" | "LOGCOUNT" | "INTEGER-LENGTH" => {
                self.compile_numeric_unary(function, span, items, name)
            }
            "FLOAT"
            | "FLOAT-SIGN"
            | "FLOAT-DIGITS"
            | "FLOAT-PRECISION"
            | "FLOAT-RADIX"
            | "DECODE-FLOAT"
            | "INTEGER-DECODE-FLOAT"
            | "LOG"
            | "ATAN"
            | "COMPLEX"
            | "SCALE-FLOAT" => self.compile_numeric_float(function, span, items, name),
            "RANDOM" => self.compile_numeric_random(function, span, items),
            "TRUNCATE" | "FLOOR" | "CEILING" | "ROUND" => {
                self.compile_numeric_rounding(function, span, items, name)
            }
            "=" | "/=" | "<" | ">" | "<=" | ">=" => {
                self.compile_numeric_comparison(function, span, items, name)
            }
            "MIN" | "MAX" | "GCD" | "LCM" | "LOGAND" | "LOGIOR" | "LOGXOR" | "+" | "*" | "-"
            | "/" => self.compile_numeric_fold(function, span, items, name),
            "MOD" | "REM" | "ASH" | "LOGTEST" | "LOGANDC1" | "LOGANDC2" | "LOGEQV" | "LOGNAND"
            | "LOGNOR" | "LOGORC1" | "LOGORC2" | "LOGBITP" | "EXPT" => {
                self.compile_numeric_binary(function, span, items, name)
            }
            "BOOLE" => self.compile_numeric_boole(function, span, items),
            "PARSE-INTEGER" => self.compile_integer_operation(function, span, items, name),
            "BYTE" | "LDB" | "MASK-FIELD" | "DPB" | "DEPOSIT-FIELD" => {
                self.compile_numeric_bitfield(function, span, items, name)
            }
            "CAR" | "CDR" | "ENDP" | "FIRST" | "REST" | "COPY-LIST" | "COPY-ALIST"
            | "LIST-LENGTH" | "VALUES-LIST" | "SECOND" | "THIRD" | "FOURTH" | "FIFTH" | "SIXTH"
            | "SEVENTH" | "EIGHTH" | "NINTH" | "TENTH" => {
                self.compile_list_unary(function, span, items, name)
            }
            "CHAR-UPCASE" | "CHAR-DOWNCASE" | "CHAR-NAME" | "NAME-CHAR" => {
                self.compile_character_unary(function, span, items, name)
            }
            "DIGIT-CHAR-P" => self.compile_character_digit_predicate(function, span, items),
            "DIGIT-CHAR" => self.compile_character_digit(function, span, items),
            "CHARACTER" | "CHAR-CODE" | "CHAR-INT" | "CODE-CHAR" | "INT-CHAR" => {
                self.compile_character_unary(function, span, items, name)
            }
            "ALPHA-CHAR-P" | "ALPHANUMERICP" | "GRAPHIC-CHAR-P" | "STANDARD-CHAR-P"
            | "UPPER-CASE-P" | "LOWER-CASE-P" | "BOTH-CASE-P" => {
                self.compile_character_predicate(function, span, items, name)
            }
            "CHAR" | "SCHAR" => self.compile_character_element(function, span, items, name),
            "CHAR=" | "CHAR<" | "CHAR>" | "CHAR<=" | "CHAR>=" | "CHAR/=" | "CHAR=/="
            | "CHAR-EQUAL" | "CHAR-LESSP" | "CHAR-GREATERP" | "CHAR-NOT-GREATERP"
            | "CHAR-NOT-LESSP" | "CHAR-NOT-EQUAL" | "CHAR-EQUALP" => {
                self.compile_character_comparison(function, span, items, name)
            }
            "STRING-UPCASE" | "STRING-DOWNCASE" | "STRING-CAPITALIZE" | "NSTRING-UPCASE"
            | "NSTRING-DOWNCASE" | "NSTRING-CAPITALIZE" => {
                self.compile_string_case(function, span, items, name)
            }
            "STRING="
            | "STRING-EQUAL"
            | "STRING<"
            | "STRING>"
            | "STRING<="
            | "STRING>="
            | "STRING/="
            | "STRING-LESSP"
            | "STRING-GREATERP"
            | "STRING-NOT-GREATERP"
            | "STRING-NOT-LESSP"
            | "STRING-NOT-EQUAL" => self.compile_string_comparison(function, span, items, name),
            "STRING-TRIM" | "STRING-LEFT-TRIM" | "STRING-RIGHT-TRIM" => {
                self.compile_string_trim(function, span, items, name)
            }
            "STRING" | "MAKE-STRING" => {
                self.compile_string_construction(function, span, items, name)
            }
            "PUTPROP" | "GET" | "GETF" | "GET-PROPERTIES" | "REMPROP" | "SYMBOL-PLIST" => {
                self.compile_property_list(function, span, items, name)
            }
            "BOUNDP" | "CONSTANTP" | "SYMBOL-VALUE" => {
                self.compile_symbol_value(function, span, items, name)
            }
            "SET" | "MAKUNBOUND" | "FMAKUNBOUND" => {
                self.compile_symbol_binding(function, span, items, name)
            }
            "FBOUNDP" | "FDEFINITION" | "SYMBOL-FUNCTION" | "MACRO-FUNCTION" => {
                self.compile_symbol_function(function, span, items, name)
            }
            "MAKE-SYMBOL" | "GENSYM" | "INTERN" | "FIND-SYMBOL" => {
                self.compile_symbol_creation(function, span, items, name)
            }
            "TERPRI"
            | "FRESH-LINE"
            | "FORCE-OUTPUT"
            | "FINISH-OUTPUT"
            | "CLEAR-OUTPUT"
            | "WRITE-CHAR"
            | "WRITE-STRING"
            | "WRITE-LINE"
            | "WRITE-SEQUENCE"
            | "READ-SEQUENCE"
            | "READ-BYTE"
            | "WRITE-BYTE"
            | "LISTEN"
            | "READ-CHAR-NO-HANG"
            | "CLEAR-INPUT"
            | "PRINC"
            | "PRIN1"
            | "PRINT"
            | "WRITE"
            | "GET-OUTPUT-STREAM-STRING"
            | "READ-CHAR"
            | "READ-LINE"
            | "PEEK-CHAR"
            | "UNREAD-CHAR"
            | "CLOSE"
            | "STREAM-ELEMENT-TYPE"
            | "STREAM-EXTERNAL-FORMAT"
            | "FILE-LENGTH"
            | "FILE-POSITION"
            | "MAKE-STRING-INPUT-STREAM"
            | "MAKE-STRING-OUTPUT-STREAM"
            | "WRITE-TO-STRING"
            | "READ-FROM-STRING"
            | "READ"
            | "READ-PRESERVING-WHITESPACE" => {
                self.compile_stream_operation(function, span, items, name)
            }
            "FIND-PACKAGE"
            | "PACKAGE-NAME"
            | "PACKAGE-USE-LIST"
            | "PACKAGE-NICKNAMES"
            | "PACKAGE-SHADOWING-SYMBOLS"
            | "PACKAGE-USED-BY-LIST" => {
                self.compile_package_introspection(function, span, items, name)
            }
            "MAKE-PACKAGE" | "DELETE-PACKAGE" | "RENAME-PACKAGE" | "USE-PACKAGE"
            | "UNUSE-PACKAGE" | "EXPORT" | "UNEXPORT" | "IMPORT" | "SHADOWING-IMPORT"
            | "SHADOW" | "UNINTERN" => self.compile_package_mutation(function, span, items, name),
            "DOCUMENTATION" | "LIST-ALL-PACKAGES" => {
                self.compile_package_listing(function, span, items, name)
            }
            "SUBTYPEP"
            | "CLASS-OF"
            | "FIND-CLASS"
            | "CLASS-NAME"
            | "CLASS-DOCUMENTATION"
            | "CLASS-PRECEDENCE-LIST"
            | "CLASS-DIRECT-SUPERCLASSES"
            | "CLASS-DIRECT-SLOTS"
            | "CLASS-SLOTS"
            | "CLASS-DEFAULT-INITARGS"
            | "CLASS-DIRECT-DEFAULT-INITARGS"
            | "CLASS-FINALIZED-P"
            | "FINALIZE-INHERITANCE"
            | "GENERIC-FUNCTION-NAME"
            | "GENERIC-FUNCTION-METHOD-COMBINATION"
            | "GENERIC-FUNCTION-LAMBDA-LIST"
            | "GENERIC-FUNCTION-DOCUMENTATION"
            | "GENERIC-FUNCTION-METHODS"
            | "ENSURE-GENERIC-FUNCTION"
            | "FIND-METHOD"
            | "ADD-METHOD"
            | "REMOVE-METHOD" => self.compile_class_introspection(function, span, items, name),
            "SLOT-VALUE"
            | "SLOT-VALUE-USING-CLASS"
            | "SLOT-EXISTS-P"
            | "SLOT-EXISTS-P-USING-CLASS"
            | "SLOT-BOUNDP"
            | "SLOT-BOUNDP-USING-CLASS"
            | "SLOT-MAKUNBOUND"
            | "SLOT-MAKUNBOUND-USING-CLASS"
            | "SLOT-DEFINITION-NAME"
            | "SLOT-DEFINITION-CLASS"
            | "SLOT-DEFINITION-DOCUMENTATION"
            | "SLOT-DEFINITION-INITARGS"
            | "SLOT-DEFINITION-ALLOCATION"
            | "SLOT-DEFINITION-INITFORM"
            | "SLOT-DEFINITION-TYPE"
            | "SLOT-DEFINITION-READERS"
            | "SLOT-DEFINITION-WRITERS" => self.compile_slot_operation(function, span, items, name),
            "ERROR" | "SIGNAL" | "WARN" | "CERROR" => {
                self.compile_condition_operation(function, span, items, name)
            }
            "COMPUTE-RESTARTS" | "RESTART-NAME" | "FIND-RESTART" | "INVOKE-RESTART" => {
                self.compile_restart_operation(function, span, items, name)
            }
            "CALL-NEXT-METHOD" | "NEXT-METHOD-P" => {
                self.compile_method_operation(function, span, items, name)
            }
            "MAKE-INSTANCE"
            | "INITIALIZE-INSTANCE"
            | "ALLOCATE-INSTANCE"
            | "CHANGE-CLASS"
            | "SHARED-INITIALIZE"
            | "REINITIALIZE-INSTANCE"
            | "UPDATE-INSTANCE-FOR-DIFFERENT-CLASS"
            | "SLOT-MISSING"
            | "SLOT-UNBOUND"
            | "COMPILE"
            | "LOAD"
            | "PROVIDE"
            | "REQUIRE" => self.compile_evaluation_operation(function, span, items, name),
            "LIST" | "LIST*" => self.compile_list_construction(function, span, items, name),
            "MAKE-LIST" => self.compile_list_construction_with_options(function, span, items),
            "LAST" | "BUTLAST" | "NBUTLAST" => self.compile_list_tail(function, span, items, name),
            "NTHCDR" | "NTH" | "TAILP" | "LDIFF" => {
                self.compile_list_binary(function, span, items, name)
            }
            "ACONS" | "PAIRLIS" | "APPEND" | "NCONC" | "REVAPPEND" | "NRECONC" => {
                self.compile_list_append(function, span, items, name)
            }
            "UNION" | "NUNION" | "INTERSECTION" | "NINTERSECTION" | "SET-DIFFERENCE"
            | "NSET-DIFFERENCE" | "SET-EXCLUSIVE-OR" | "NSET-EXCLUSIVE-OR" | "SUBSETP" => {
                self.compile_list_set(function, span, items, name)
            }
            "TREE-EQUAL" => self.compile_tree_equal(function, span, items),
            "OPEN" => self.compile_file_operation(function, span, items, name),
            "PROBE-FILE" | "RENAME-FILE" => {
                self.compile_file_metadata_operation(function, span, items, name)
            }
            _ => return None,
        };
        Some(result)
    }
}
