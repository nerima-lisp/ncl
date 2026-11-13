use super::*;

const MAX_FIXNUM: u128 = (1_u128 << 62) - 1;

fn function_word(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    runtime
        .function(ctx, "NCL-GC", name)
        .filter(|function| crate::FunctionObject::try_from(*function).is_ok())
        .unwrap_or_else(|| panic!("NCL-GC:{name} builtin was not registered"))
}

fn call_builtin(ctx: &mut ThreadContext, runtime: &Runtime, function: Word) -> Word {
    runtime
        .call_builtin(
            ctx,
            crate::FunctionObject::try_from(function)
                .unwrap_or_else(|error| panic!("function word: {error:?}")),
            &[],
        )
        .unwrap_or_else(|error| panic!("NCL-GC counter call failed: {error:?}"))
}

fn call_counter(ctx: &mut ThreadContext, runtime: &Runtime, function: Word) -> i64 {
    call_builtin(ctx, runtime, function)
        .as_fixnum()
        .unwrap_or_else(|| panic!("NCL-GC counter did not return a fixnum"))
}

#[test]
fn counter_word_keeps_integer_values_exact_past_the_fixnum_limit() {
    // CLHS System Class INTEGER states there is no magnitude limit and that fixnum and bignum
    // partition integers: https://www.lispworks.com/documentation/HyperSpec/Body/t_intege.htm.
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = crate::ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));

    let fixnum = counter_word(&mut ctx, &runtime, MAX_FIXNUM)
        .unwrap_or_else(|error| panic!("fixnum counter: {error:?}"));
    let max_fixnum = i64::try_from(MAX_FIXNUM)
        .unwrap_or_else(|error| panic!("fixnum limit does not fit in i64: {error}"));
    assert_eq!(fixnum.as_fixnum(), Some(max_fixnum));

    let bignum = counter_word(&mut ctx, &runtime, MAX_FIXNUM + 1)
        .unwrap_or_else(|error| panic!("bignum counter: {error:?}"));
    let crate::ObjectRef::Bignum(word) = crate::classify_object(&ctx, bignum) else {
        panic!("counter above the fixnum limit was not a bignum");
    };
    let object = crate::Bignum::from_word(word);
    assert!(!crate::bignum_sign(&ctx, object).unwrap_or_else(|error| panic!("sign: {error:?}")));
    assert_eq!(
        crate::bignum_limbs(&ctx, object).unwrap_or_else(|error| panic!("limbs: {error:?}")),
        vec![0, 1_u32 << 30]
    );
}

#[test]
fn gc_builtin_returns_thread_not_registered_for_an_unregistered_context() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = crate::ThreadContext::new();
    let args = BuiltinArgs::new(&[]);
    let mut values = MultipleValues::new();

    let Err(error) = gc_builtin(&mut ctx, &runtime, &args, &mut values) else {
        panic!("unregistered context must be rejected");
    };

    assert_eq!(
        error,
        ObjectError::Storage(ncl_sys::StorageCondition::ThreadNotRegistered)
    );
    assert_eq!(error.to_string(), "Storage(ThreadNotRegistered)");
}

#[test]
fn counter_word_rejects_values_above_the_bignum_range() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = crate::ThreadContext::new();
    let too_large = i128::MAX.unsigned_abs() + 1;

    let Err(error) = counter_word(&mut ctx, &runtime, too_large) else {
        panic!("out-of-range allocation counter must be rejected");
    };

    assert_eq!(error, ObjectError::Layout);
    assert_eq!(error.to_string(), "Layout");
}

#[test]
fn gc_layout_registration_rejects_a_duplicate_widetag() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));

    let Err(error) = register_layouts(&runtime) else {
        panic!("duplicate widetag registration must be rejected");
    };

    assert_eq!(error, ObjectError::Layout);
    assert_eq!(error.to_string(), "Layout");
}

#[test]
fn gc_registration_returns_type_error_without_context_registration() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = crate::ThreadContext::new();

    let Err(error) = register(&mut ctx, &runtime) else {
        panic!("unregistered context must be rejected");
    };

    assert_eq!(error, ObjectError::TypeError);
    assert_eq!(error.to_string(), "TypeError");
}

#[test]
fn gc_registration_preserves_the_locked_package_lisp_error() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = crate::ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register context: {error:?}"));
    let package = runtime
        .ensure_package(&mut ctx, "NCL-GC")
        .unwrap_or_else(|error| panic!("ensure NCL-GC package: {error:?}"));
    crate::Package::from_word(package)
        .set_locked(&mut ctx, true)
        .unwrap_or_else(|error| panic!("lock NCL-GC package: {error:?}"));

    let Err(error) = register(&mut ctx, &runtime) else {
        panic!("locked package registration must be rejected");
    };

    assert_eq!(error, ObjectError::TypeError);
    assert_eq!(error.to_string(), "TypeError");
    assert_eq!(
        ctx.take_pending_lisp_error(),
        Some(crate::LispError::PackageError(crate::PackageError::Locked))
    );
}

#[test]
fn allocation_debt_and_gc_count_are_read_only_counters() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = crate::ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    register(&mut ctx, &runtime).unwrap_or_else(|error| panic!("NCL-GC register: {error:?}"));

    let mut functions = [Word::NIL; 3];
    let roots = ctx.thread_access().register_root_set(&mut functions);
    functions[0] = function_word(&mut ctx, &runtime, "GC");
    functions[1] = function_word(&mut ctx, &runtime, "BYTES-CONSED-SINCE-GC");
    functions[2] = function_word(&mut ctx, &runtime, "GC-COUNT");
    let before_collection = call_counter(&mut ctx, &runtime, functions[2]);
    let _ = call_builtin(&mut ctx, &runtime, functions[0]);
    assert_eq!(
        call_counter(&mut ctx, &runtime, functions[2]),
        before_collection + 1
    );
    assert_eq!(call_counter(&mut ctx, &runtime, functions[1]), 0);

    crate::allocate(&mut ctx, &runtime, widetag::BIGNUM, 3)
        .unwrap_or_else(|error| panic!("heap allocation: {error:?}"));
    assert_eq!(call_counter(&mut ctx, &runtime, functions[1]), 32);
    assert_eq!(
        call_counter(&mut ctx, &runtime, functions[2]),
        before_collection + 1
    );

    let _ = call_builtin(&mut ctx, &runtime, functions[0]);
    assert_eq!(
        call_counter(&mut ctx, &runtime, functions[2]),
        before_collection + 2
    );
    assert_eq!(call_counter(&mut ctx, &runtime, functions[1]), 0);
    assert!(ctx.thread_access().pop_root(roots));
}

#[test]
fn package_layout_uses_header_inclusive_references() {
    assert_eq!(
        package_layout().reference_words,
        reference_words(&crate::package::reference_words())
    );
    assert!(
        package_layout()
            .reference_words
            .contains(&(crate::package::DOCUMENTATION + 1))
    );
    assert_eq!(package_layout().boxed_from, Some(crate::package::NAME + 1));
}

#[test]
fn gc_builtin_runs_a_full_collection() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = crate::ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    register(&mut ctx, &runtime).unwrap_or_else(|error| panic!("NCL-GC register: {error:?}"));
    let gc = runtime
        .function(&mut ctx, "NCL-GC", "GC")
        .and_then(|function| crate::FunctionObject::try_from(function).ok())
        .unwrap_or_else(|| panic!("NCL-GC:GC builtin was not registered"));
    let before = ncl_sys::heap_epoch(&ctx.thread);

    runtime
        .call_builtin(&mut ctx, gc, &[])
        .unwrap_or_else(|error| panic!("NCL-GC:GC failed: {error:?}"));

    let after = ncl_sys::heap_epoch(&ctx.thread);
    assert!(
        after > before,
        "NCL-GC:GC did not advance the collection epoch"
    );
}
