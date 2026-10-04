use super::*;
use ncl_object::{FunctionObject, Package};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut context = ThreadContext::new();
    context.register(&runtime).expect("context");
    crate::register(&runtime).expect("clos registration");
    (runtime, context)
}

fn intern(context: &mut ThreadContext, runtime: &Runtime, package: &str, name: &str) -> Word {
    Package::from_word(runtime.find_package(context, package).expect("package"))
        .intern(context, runtime, name)
        .expect("symbol")
        .0
}

fn list(context: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    let mut scope = Scope::new(context);
    let roots = scope.root_many(
        &values
            .iter()
            .copied()
            .map(Local::from_word)
            .collect::<Vec<_>>(),
    );
    let result = scope.make_list(runtime, &roots).expect("list");
    scope.get(result).as_word()
}

fn elements(context: &ThreadContext, mut value: Word) -> Result<Vec<Word>, ObjectError> {
    let mut result = Vec::new();
    while value != Word::NIL {
        result.push(car(context, value)?);
        value = cdr(context, value)?;
    }
    Ok(result)
}

fn call_macro(
    context: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    form: Word,
) -> Result<Word, ObjectError> {
    let function = FunctionObject::try_from(
        runtime
            .function(context, COMMON_LISP, name)
            .expect("macro function"),
    )
    .expect("function object");
    runtime.call_builtin(context, function, &[form])
}

fn assert_type_error(result: Result<Word, ObjectError>) {
    let error = result.expect_err("expected TypeError");
    assert_eq!(error, ObjectError::TypeError);
    assert_eq!(error.to_string(), "TypeError");
}

#[test]
fn macro_list_to_handles_returns_values_and_rejects_dotted_lists() {
    let (runtime, mut context) = setup();
    {
        let mut scope = Scope::new(&mut context);
        let values = scope.root_many(
            &[Word::fixnum(3), Word::fixnum(5)]
                .iter()
                .copied()
                .map(Local::from_word)
                .collect::<Vec<_>>(),
        );
        let proper = scope.make_list(&runtime, &values).expect("proper list");
        let handles = macro_list_to_handles(&mut scope, proper).expect("list handles");
        assert_eq!(
            handles
                .iter()
                .map(|handle| scope.get(*handle).as_word())
                .collect::<Vec<_>>(),
            vec![Word::fixnum(3), Word::fixnum(5)]
        );
    }

    let dotted = ncl_object::make_cons(&mut context, &runtime, Word::fixnum(3), Word::fixnum(5))
        .expect("dotted list");
    let mut dotted_scope = Scope::new(&mut context);
    let dotted = dotted_scope.root(Local::from_word(dotted));
    let error =
        macro_list_to_handles(&mut dotted_scope, dotted).expect_err("dotted list must fail");
    assert_eq!(error, ObjectError::TypeError);
    assert_eq!(error.to_string(), "TypeError");
}

#[test]
fn defgeneric_expansion_has_exact_progn_shape_and_quoted_result() {
    let (runtime, mut context) = setup();
    let name = intern(
        &mut context,
        &runtime,
        "COMMON-LISP-USER",
        "MACRO-EXACT-GENERIC",
    );
    let defgeneric = intern(&mut context, &runtime, COMMON_LISP, "DEFGENERIC");
    let form = list(&mut context, &runtime, &[defgeneric, name]);
    let expansion =
        call_macro(&mut context, &runtime, "DEFGENERIC", form).expect("defgeneric expansion");
    let values = elements(&context, expansion).expect("progn elements");
    assert_eq!(values.len(), 4);
    assert_eq!(
        values[0],
        intern(&mut context, &runtime, COMMON_LISP, "PROGN")
    );
    let clear = elements(&context, values[1]).expect("clear form");
    assert_eq!(
        clear[0],
        intern(&mut context, &runtime, COMMON_LISP, "%CLOS-DEFINE-GENERIC")
    );
    assert_eq!(
        elements(&context, clear[1]).expect("quoted name"),
        vec![intern(&mut context, &runtime, COMMON_LISP, "QUOTE"), name,]
    );
    let function = elements(&context, values[2]).expect("defun form");
    assert_eq!(function[1], name);
    assert_eq!(
        elements(&context, values[3]).expect("result"),
        vec![intern(&mut context, &runtime, COMMON_LISP, "QUOTE"), name,]
    );
}

#[test]
fn defmethod_accepts_rest_next_forms_and_returns_expansion() {
    let (runtime, mut context) = setup();
    let defmethod = intern(&mut context, &runtime, COMMON_LISP, "DEFMETHOD");
    let name = intern(
        &mut context,
        &runtime,
        "COMMON-LISP-USER",
        "MACRO-REST-METHOD",
    );
    let rest = intern(&mut context, &runtime, COMMON_LISP, "&REST");
    let args = intern(&mut context, &runtime, "COMMON-LISP-USER", "ARGS");
    let value = intern(&mut context, &runtime, "COMMON-LISP-USER", "VALUE");
    let integer = intern(&mut context, &runtime, COMMON_LISP, "INTEGER");
    let call_next = intern(&mut context, &runtime, COMMON_LISP, "CALL-NEXT-METHOD");
    let quote = intern(&mut context, &runtime, COMMON_LISP, "QUOTE");
    let quoted_value = list(&mut context, &runtime, &[quote, Word::fixnum(9)]);
    let call_next_form = list(&mut context, &runtime, &[call_next]);
    let parameter = list(&mut context, &runtime, &[value, integer]);
    let specializer = list(&mut context, &runtime, &[parameter, rest, args]);
    let form = list(
        &mut context,
        &runtime,
        &[defmethod, name, specializer, quoted_value, call_next_form],
    );

    let expansion =
        call_macro(&mut context, &runtime, "DEFMETHOD", form).expect("rest method expansion");
    let values = elements(&context, expansion).expect("defmethod progn expansion");
    assert_eq!(values.len(), 5);
    assert_eq!(values[0], intern(&mut context, &runtime, COMMON_LISP, "PROGN"));
    assert_eq!(
        elements(&context, values[4]).expect("quoted method name"),
        vec![intern(&mut context, &runtime, COMMON_LISP, "QUOTE"), name]
    );
}

#[test]
fn macro_entry_points_report_type_error_for_empty_and_malformed_forms() {
    let (runtime, mut context) = setup();
    for name in ["DEFCLASS", "DEFGENERIC", "DEFMETHOD"] {
        assert_type_error(call_macro(&mut context, &runtime, name, Word::NIL));
    }

    let defclass = intern(&mut context, &runtime, COMMON_LISP, "DEFCLASS");
    let class_name = intern(
        &mut context,
        &runtime,
        "COMMON-LISP-USER",
        "MACRO-BAD-CLASS",
    );
    let dotted =
        ncl_object::make_cons(&mut context, &runtime, defclass, class_name).expect("dotted form");
    assert_type_error(call_macro(&mut context, &runtime, "DEFCLASS", dotted));

    let defmethod = intern(&mut context, &runtime, COMMON_LISP, "DEFMETHOD");
    let method_name = intern(
        &mut context,
        &runtime,
        "COMMON-LISP-USER",
        "MACRO-BAD-METHOD",
    );
    let invalid_specializer = list(&mut context, &runtime, &[Word::fixnum(1)]);
    let invalid_form = list(
        &mut context,
        &runtime,
        &[defmethod, method_name, invalid_specializer],
    );
    assert_type_error(call_macro(
        &mut context,
        &runtime,
        "DEFMETHOD",
        invalid_form,
    ));
}
