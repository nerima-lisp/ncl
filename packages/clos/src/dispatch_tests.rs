#[cfg(test)]
mod included_tests {
use super::super::COMMON_LISP;
use ncl_object::{FunctionObject, Package, Runtime, ThreadContext, Word, push_heap_root};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut context = ThreadContext::new();
    context.register(&runtime).expect("context");
    super::super::register(&runtime).expect("clos registration");
    (runtime, context)
}

fn list(context: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    let mut scope = ncl_object::Scope::new(context);
    let roots = scope.root_many(
        &values
            .iter()
            .copied()
            .map(ncl_object::Local::from_word)
            .collect::<Vec<_>>(),
    );
    let result = scope.make_list(runtime, &roots).expect("list");
    scope.get(result).as_word()
}

#[test]
fn generic_dispatch_survives_gc_forced_on_every_allocation() {
    let (runtime, mut context) = setup();
    let package = runtime
        .find_package(&context, COMMON_LISP)
        .expect("package");
    let (mut generic, _) = Package::from_word(package)
        .intern(&mut context, &runtime, "GC-STRESS-DISPATCH")
        .expect("generic symbol");
    let class = Package::from_word(package)
        .intern(&mut context, &runtime, "INTEGER")
        .expect("class symbol")
        .0;
    let variable = Package::from_word(runtime.find_package(&context, "NCL").expect("ncl"))
        .intern(&mut context, &runtime, "VALUE")
        .expect("variable symbol")
        .0;
    let specializer = list(&mut context, &runtime, &[variable, class]);
    let mut specializers = list(&mut context, &runtime, &[specializer]);
    let mut arguments = list(&mut context, &runtime, &[Word::fixnum(7)]);
    let define = FunctionObject::try_from(
        runtime
            .function(&mut context, COMMON_LISP, "%CLOS-DEFINE-GENERIC")
            .expect("define builtin"),
    )
    .expect("define function");
    let add = FunctionObject::try_from(
        runtime
            .function(&mut context, COMMON_LISP, "%CLOS-ADD-METHOD")
            .expect("add builtin"),
    )
    .expect("add function");
    let dispatch = FunctionObject::try_from(
        runtime
            .function(&mut context, COMMON_LISP, "%CLOS-DISPATCH")
            .expect("dispatch builtin"),
    )
    .expect("dispatch function");
    let method = FunctionObject::try_from(
        runtime
            .function(&mut context, COMMON_LISP, "CLASS-OF")
            .expect("method builtin"),
    )
    .expect("method function");
    let mut define_word = define.as_word();
    let mut add_word = add.as_word();
    let mut dispatch_word = dispatch.as_word();
    let mut method_word = method.as_word();
    let roots = [
        push_heap_root(&runtime, &mut generic),
        push_heap_root(&runtime, &mut specializers),
        push_heap_root(&runtime, &mut arguments),
        push_heap_root(&runtime, &mut define_word),
        push_heap_root(&runtime, &mut add_word),
        push_heap_root(&runtime, &mut dispatch_word),
        push_heap_root(&runtime, &mut method_word),
    ];
    context.set_gc_stress(false);
    context.set_strict_forwarding(true);
    runtime
        .call_builtin(&mut context, define, &[generic])
        .expect("define");
    runtime
        .call_builtin(&mut context, add, &[generic, specializers, method_word])
        .expect("add");
    context.set_gc_stress(true);
    let result = runtime
        .call_builtin(&mut context, dispatch, &[generic, arguments])
        .expect("dispatch");
    let expected_class = runtime
        .class(&mut context, "INTEGER")
        .expect("class descriptor");
    let expected = super::super::class_name(&context, expected_class).expect("class descriptor");
    assert_eq!(
        super::super::class_name(&context, result).expect("class"),
        expected,
    );
    let _ = roots;
}

#[test]
fn call_next_method_chain_survives_gc_forced_on_every_allocation() {
    let (runtime, mut context) = setup();
    let class_of = FunctionObject::try_from(
        runtime
            .function(&mut context, COMMON_LISP, "CLASS-OF")
            .expect("class-of"),
    )
    .expect("class-of function");
    let next_methods = list(&mut context, &runtime, &[class_of.as_word()]);
    let supplied = list(&mut context, &runtime, &[Word::fixnum(7)]);
    let call_next = FunctionObject::try_from(
        runtime
            .function(&mut context, COMMON_LISP, "%CLOS-CALL-NEXT-METHOD")
            .expect("call-next-method"),
    )
    .expect("call-next-method function");
    let mut next_word = next_methods;
    let mut supplied_word = supplied;
    let next_token = push_heap_root(&runtime, &mut next_word);
    let supplied_token = push_heap_root(&runtime, &mut supplied_word);
    context.set_gc_stress(true);
    context.set_strict_forwarding(true);
    let mut result = runtime
        .call_builtin(&mut context, call_next, &[next_word, supplied_word])
        .expect("call-next-method");
    let result_token = push_heap_root(&runtime, &mut result);
    let integer = runtime.class(&mut context, "INTEGER").expect("integer");
    let expected = super::super::class_name(&context, integer).expect("class descriptor");
    assert_eq!(
        super::super::class_name(&context, result).expect("class"),
        expected,
    );
    let _ = (result_token, supplied_token, next_token);
}

#[test]
fn dispatch_specificity_uses_argument_precedence_and_eql_descriptors() {
    let (runtime, mut context) = setup();
    let package = runtime
        .find_package(&context, COMMON_LISP)
        .expect("package");
    let (variable, _) = Package::from_word(package)
        .intern(&mut context, &runtime, "VALUE")
        .expect("variable");
    let (integer, _) = Package::from_word(package)
        .intern(&mut context, &runtime, "INTEGER")
        .expect("integer");
    let (t_class, _) = Package::from_word(package)
        .intern(&mut context, &runtime, "T")
        .expect("t class");
    let eql = super::super::mop::make_eql_specializer(
        &mut context,
        &runtime,
        Word::fixnum(7),
    )
    .expect("eql specializer");
    let first_integer = list(&mut context, &runtime, &[variable, integer]);
    let first_t = list(&mut context, &runtime, &[variable, t_class]);
    let second_eql = list(&mut context, &runtime, &[variable, eql]);
    let second_t = list(&mut context, &runtime, &[variable, t_class]);
    let first = list(&mut context, &runtime, &[first_integer, second_t]);
    let second = list(&mut context, &runtime, &[first_t, second_eql]);

    let first_score = super::super::dispatch_method_match(
        &mut context,
        &runtime,
        first,
        &[Word::fixnum(7), Word::fixnum(7)],
    )
    .expect("first match")
    .expect("first applicable");
    let second_score = super::super::dispatch_method_match(
        &mut context,
        &runtime,
        second,
        &[Word::fixnum(7), Word::fixnum(7)],
    )
    .expect("second match")
    .expect("second applicable");

    assert_eq!(first_score, vec![2, 0]);
    assert_eq!(second_score, vec![0, usize::MAX]);
    assert!(first_score > second_score);
}
}
