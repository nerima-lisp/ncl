#![allow(missing_docs)]
#![allow(
    clippy::unwrap_used,
    reason = "tests assert on macro expansion contracts"
)]

use ncl_object::{FunctionObject, Package, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();
    (runtime, ctx)
}

fn intern(runtime: &Runtime, ctx: &mut ThreadContext, package: &str, name: &str) -> Word {
    let package = runtime.find_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    let mut scope = ncl_object::Scope::new(ctx);
    let roots = scope.root_many(
        &values
            .iter()
            .copied()
            .map(ncl_object::Local::from_word)
            .collect::<Vec<_>>(),
    );
    let result = scope.make_list(runtime, &roots).unwrap();
    scope.get(result).as_word()
}

fn contains(ctx: &ThreadContext, value: Word, target: Word) -> bool {
    value == target
        || (value.is_cons()
            && (contains(ctx, ncl_object::car(ctx, value).unwrap(), target)
                || contains(ctx, ncl_object::cdr(ctx, value).unwrap(), target)))
}

fn call_macro(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, form: Word) -> Word {
    let function =
        FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap();
    runtime.call_builtin(ctx, function, &[form]).unwrap()
}

#[test]
fn defgeneric_expands_to_registration_dispatch_and_quoted_result() {
    let (runtime, mut ctx) = setup();
    let defgeneric = intern(&runtime, &mut ctx, "COMMON-LISP", "DEFGENERIC");
    let name = intern(
        &runtime,
        &mut ctx,
        "COMMON-LISP-USER",
        "MACRO-COVERAGE-GENERIC",
    );
    let rest = intern(&runtime, &mut ctx, "COMMON-LISP", "&REST");
    let args = intern(&runtime, &mut ctx, "COMMON-LISP-USER", "ARGS");
    let lambda_list = list(&mut ctx, &runtime, &[rest, args]);
    let form = list(&mut ctx, &runtime, &[defgeneric, name, lambda_list]);

    let expansion = call_macro(&runtime, &mut ctx, "DEFGENERIC", form);
    let define = intern(&runtime, &mut ctx, "COMMON-LISP", "%CLOS-DEFINE-GENERIC");
    let dispatch = intern(&runtime, &mut ctx, "COMMON-LISP", "%CLOS-DISPATCH");
    assert!(contains(&ctx, expansion, define));
    assert!(contains(&ctx, expansion, dispatch));
    assert!(contains(&ctx, expansion, name));
}

#[test]
fn defmethod_rewrites_next_method_forms_and_keeps_specializer_metadata() {
    let (runtime, mut ctx) = setup();
    let defmethod = intern(&runtime, &mut ctx, "COMMON-LISP", "DEFMETHOD");
    let name = intern(
        &runtime,
        &mut ctx,
        "COMMON-LISP-USER",
        "MACRO-COVERAGE-METHOD",
    );
    let value = intern(&runtime, &mut ctx, "COMMON-LISP-USER", "VALUE");
    let integer = intern(&runtime, &mut ctx, "COMMON-LISP", "INTEGER");
    let call_next = intern(&runtime, &mut ctx, "COMMON-LISP", "CALL-NEXT-METHOD");
    let next_p = intern(&runtime, &mut ctx, "COMMON-LISP", "NEXT-METHOD-P");
    let parameter = list(&mut ctx, &runtime, &[value, integer]);
    let specializer = list(&mut ctx, &runtime, &[parameter]);
    let call_next_form = list(&mut ctx, &runtime, &[call_next]);
    let next_p_form = list(&mut ctx, &runtime, &[next_p]);
    let form = list(
        &mut ctx,
        &runtime,
        &[defmethod, name, specializer, call_next_form, next_p_form],
    );

    let expansion = call_macro(&runtime, &mut ctx, "DEFMETHOD", form);
    let call_next_impl = intern(&runtime, &mut ctx, "COMMON-LISP", "%CLOS-CALL-NEXT-METHOD");
    let next_p_impl = intern(&runtime, &mut ctx, "COMMON-LISP", "%CLOS-NEXT-METHOD-P");
    let definition_tag = intern(&runtime, &mut ctx, "NCL", "*CLOS-METHOD-DEFINITION*");
    assert!(contains(&ctx, expansion, value));
    assert!(contains(&ctx, expansion, call_next_impl));
    assert!(contains(&ctx, expansion, next_p_impl));
    assert!(contains(&ctx, expansion, definition_tag));
}

#[test]
fn defclass_and_qualified_defmethod_preserve_slot_and_method_contracts() {
    let (runtime, mut ctx) = setup();
    let defclass = intern(&runtime, &mut ctx, "COMMON-LISP", "DEFCLASS");
    let class_name = intern(
        &runtime,
        &mut ctx,
        "COMMON-LISP-USER",
        "MACRO-COVERAGE-CLASS",
    );
    let slot_name = intern(&runtime, &mut ctx, "COMMON-LISP-USER", "VALUE");
    let initarg = intern(&runtime, &mut ctx, "COMMON-LISP", ":VALUE");
    let initform = intern(&runtime, &mut ctx, "COMMON-LISP", "T");
    let accessor = intern(
        &runtime,
        &mut ctx,
        "COMMON-LISP-USER",
        "MACRO-COVERAGE-VALUE",
    );
    let initarg_key = intern(&runtime, &mut ctx, "COMMON-LISP", ":INITARG");
    let initform_key = intern(&runtime, &mut ctx, "COMMON-LISP", ":INITFORM");
    let accessor_key = intern(&runtime, &mut ctx, "COMMON-LISP", ":ACCESSOR");
    let slot = list(
        &mut ctx,
        &runtime,
        &[
            slot_name,
            initarg_key,
            initarg,
            initform_key,
            initform,
            accessor_key,
            accessor,
        ],
    );
    let slots = list(&mut ctx, &runtime, &[slot]);
    let empty_supers = Word::NIL;
    let class_form = list(
        &mut ctx,
        &runtime,
        &[defclass, class_name, empty_supers, slots],
    );

    let class_expansion = call_macro(&runtime, &mut ctx, "DEFCLASS", class_form);
    assert!(contains(&ctx, class_expansion, slot_name));
    assert!(contains(&ctx, class_expansion, accessor));

    let defmethod = intern(&runtime, &mut ctx, "COMMON-LISP", "DEFMETHOD");
    let around = intern(&runtime, &mut ctx, "COMMON-LISP", ":AROUND");
    let integer = intern(&runtime, &mut ctx, "COMMON-LISP", "INTEGER");
    let initialize = intern(&runtime, &mut ctx, "COMMON-LISP", "INITIALIZE-INSTANCE");
    let call_next = intern(&runtime, &mut ctx, "COMMON-LISP", "CALL-NEXT-METHOD");
    let parameter = list(&mut ctx, &runtime, &[slot_name, integer]);
    let specializer = list(&mut ctx, &runtime, &[parameter]);
    let explicit_call = list(&mut ctx, &runtime, &[call_next, Word::fixnum(17)]);
    let method_form = list(
        &mut ctx,
        &runtime,
        &[defmethod, initialize, around, specializer, explicit_call],
    );

    let method_expansion = call_macro(&runtime, &mut ctx, "DEFMETHOD", method_form);
    let ensure_base = intern(
        &runtime,
        &mut ctx,
        "COMMON-LISP",
        "%CLOS-ENSURE-INITIALIZATION-BASE",
    );
    let call_next_impl = intern(&runtime, &mut ctx, "COMMON-LISP", "%CLOS-CALL-NEXT-METHOD");
    assert!(contains(&ctx, method_expansion, ensure_base));
    assert!(contains(&ctx, method_expansion, call_next_impl));
    assert!(contains(&ctx, method_expansion, Word::fixnum(17)));
}
