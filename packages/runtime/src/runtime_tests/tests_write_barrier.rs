use super::{FunctionArguments, FunctionCaller, MultipleValues, Package, Runtime, symbol_value};

pub(super) fn run() {
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    runtime.set_gc_stress(false);
    runtime.set_strict_forwarding(true);

    let pair_symbol = rooted_user_symbol(&mut runtime, "*U03-STOREFIELD-PAIR*");
    let pair_recovery_symbol = rooted_user_symbol(&mut runtime, "*U03-STOREFIELD-RECOVERED*");
    let pair_weak_symbol = rooted_user_symbol(&mut runtime, "*U03-STOREFIELD-WEAK-PAIR*");
    let cell_weak_symbol = rooted_user_symbol(&mut runtime, "*U03-STOREFIELD-WEAK-CELL*");
    prepare_old_storefield_cell(
        &mut runtime,
        &pair_symbol,
        &pair_weak_symbol,
        &cell_weak_symbol,
    );

    let young = runtime
        .eval("(cons 701 702)")
        .unwrap_or_else(|error| panic!("JIT argument cons creation failed: {error}"));
    let pair = symbol_value(&runtime.context, pair_symbol.get())
        .unwrap_or_else(|error| panic!("captured cell pair lookup before call failed: {error}"));
    let setter = storefield_setter(&runtime, pair, "captured");
    let setter = ncl_object::FunctionObject::try_from(setter)
        .unwrap_or_else(|error| panic!("captured setter conversion failed: {error}"));
    let arguments = [young];
    let mut values = MultipleValues::new();
    let mut caller = runtime.function_caller();
    // Exercise FunctionCaller after eval returns to its prior native context.
    let jit_store = caller
        .call_function(
            &mut runtime.context,
            &runtime.object,
            ncl_object::typed::FunctionDesignator::Function(setter),
            FunctionArguments::new(&arguments),
            &mut values,
        )
        .map_err(|error| error.to_string());
    assert_eq!(jit_store, Ok(ncl_sys::Word::fixnum(701)));
    ncl_object::set_symbol_value(&mut runtime.context, pair_symbol.get(), ncl_sys::Word::NIL)
        .unwrap_or_else(|error| panic!("captured cell pair unrooting failed: {error}"));
    runtime
        .eval("NIL")
        .unwrap_or_else(|error| panic!("clearing the setter return value failed: {error}"));
    collect_minor_twice(&mut runtime, "post-store");

    let weak_cell = weak_target(&runtime, cell_weak_symbol.get())
        .unwrap_or_else(|error| panic!("weak cell lookup failed: {error}"));
    let pair = restore_weak_target(
        &mut runtime,
        pair_weak_symbol.get(),
        pair_recovery_symbol.get(),
    )
    .unwrap_or_else(|error| panic!("weak pair recovery failed: {error}"));
    let captured_cell = storefield_setter_cell(&runtime, pair, "recovered");
    assert_eq!(
        captured_cell, weak_cell,
        "weak link must name the StoreField owner"
    );
    let jit_read = read_recovered_cell_car(&mut runtime, "*U03-STOREFIELD-RECOVERED*");

    let api_read = read_after_barriered_api_store(
        &mut runtime,
        weak_cell,
        &pair_weak_symbol,
        &pair_recovery_symbol,
    )
    .unwrap_or_else(|error| panic!("positive-control failed: {error}"));

    assert_eq!(api_read, ncl_sys::Word::fixnum(801));
    assert_eq!(jit_read, Ok(ncl_sys::Word::fixnum(701)));
}

fn prepare_old_storefield_cell(
    runtime: &mut Runtime,
    pair_symbol: &ncl_object::HeapRootedWord,
    pair_weak_symbol: &ncl_object::HeapRootedWord,
    cell_weak_symbol: &ncl_object::HeapRootedWord,
) {
    let pair = runtime
        .eval(
            "(let ((cell nil)) \
               (list (lambda () cell) \
                     (lambda (value) (progn (setq cell value) (car value)))))",
        )
        .unwrap_or_else(|error| panic!("captured cell pair creation failed: {error}"));
    ncl_object::set_symbol_value(&mut runtime.context, pair_symbol.get(), pair)
        .unwrap_or_else(|error| panic!("captured cell pair rooting failed: {error}"));
    runtime
        .eval("NIL")
        .unwrap_or_else(|error| panic!("clearing the pair return value failed: {error}"));
    collect_minor_twice(runtime, "promotion");

    let pair = symbol_value(&runtime.context, pair_symbol.get())
        .unwrap_or_else(|error| panic!("captured cell pair lookup failed: {error}"));
    let cell = storefield_setter_cell(runtime, pair, "captured");
    set_weak_link(runtime, pair_weak_symbol.get(), pair);
    set_weak_link(runtime, cell_weak_symbol.get(), cell);
}

fn storefield_setter_cell(runtime: &Runtime, pair: ncl_sys::Word, phase: &str) -> ncl_sys::Word {
    let setter = storefield_setter(runtime, pair, phase);
    ncl_object::closure_ref(&runtime.context, ncl_object::Function::from_word(setter), 0)
        .unwrap_or_else(|error| panic!("{phase} value cell lookup failed: {error}"))
}

fn storefield_setter(runtime: &Runtime, pair: ncl_sys::Word, phase: &str) -> ncl_sys::Word {
    let setter_list = ncl_object::cdr(&runtime.context, pair)
        .unwrap_or_else(|error| panic!("{phase} setter list lookup failed: {error}"));
    ncl_object::car(&runtime.context, setter_list)
        .unwrap_or_else(|error| panic!("{phase} setter lookup failed: {error}"))
}

fn collect_minor_twice(runtime: &mut Runtime, phase: &str) {
    for collection in [1, 2] {
        runtime.context.collect(false).unwrap_or_else(|error| {
            panic!("{phase} minor collection {collection} failed: {error}")
        });
    }
}

fn read_after_barriered_api_store(
    runtime: &mut Runtime,
    weak_cell: ncl_sys::Word,
    pair_weak_symbol: &ncl_object::HeapRootedWord,
    pair_recovery_symbol: &ncl_object::HeapRootedWord,
) -> Result<ncl_sys::Word, String> {
    let api_value = runtime
        .eval("(cons 801 802)")
        .map_err(|error| format!("positive-control cons creation failed: {error}"))?;
    runtime
        .context
        .write_object_slot(weak_cell, 0, api_value)
        .map_err(|error| format!("positive-control cell store failed: {error}"))?;
    runtime
        .eval("NIL")
        .map_err(|error| format!("clearing the positive-control value failed: {error}"))?;
    ncl_object::set_symbol_value(
        &mut runtime.context,
        pair_recovery_symbol.get(),
        ncl_sys::Word::NIL,
    )
    .map_err(|error| format!("recovered pair unrooting failed: {error}"))?;
    collect_minor_twice(runtime, "positive-control");
    restore_weak_target(runtime, pair_weak_symbol.get(), pair_recovery_symbol.get())
        .map_err(|error| format!("positive-control pair recovery failed: {error}"))?;
    read_recovered_cell_car(runtime, "*U03-STOREFIELD-RECOVERED*")
}

fn intern_user_symbol(runtime: &mut Runtime, name: &str) -> ncl_sys::Word {
    let package = runtime
        .object
        .find_package(&runtime.context, "COMMON-LISP-USER")
        .unwrap_or_else(|| panic!("COMMON-LISP-USER package missing"));
    Package::from_word(package)
        .intern(&mut runtime.context, &runtime.object, name)
        .unwrap_or_else(|error| panic!("symbol {name} interning failed: {error}"))
        .0
}

fn rooted_user_symbol(runtime: &mut Runtime, name: &str) -> ncl_object::HeapRootedWord {
    let symbol = intern_user_symbol(runtime, name);
    ncl_object::HeapRootedWord::new(&runtime.context, symbol)
        .unwrap_or_else(|error| panic!("symbol {name} rooting failed: {error}"))
}

fn set_weak_link(runtime: &mut Runtime, symbol: ncl_sys::Word, owner: ncl_sys::Word) {
    let link = ncl_object::make_cons(
        &mut runtime.context,
        &runtime.object,
        ncl_sys::Word::NIL,
        owner,
    )
    .unwrap_or_else(|error| panic!("weak link allocation failed: {error}"));
    let weak = runtime.context.make_weak(link, ncl_sys::Weakness::Value);
    ncl_object::set_symbol_value(&mut runtime.context, symbol, weak)
        .unwrap_or_else(|error| panic!("weak link rooting failed: {error}"));
}

fn weak_target(runtime: &Runtime, symbol: ncl_sys::Word) -> Result<ncl_sys::Word, String> {
    let link = symbol_value(&runtime.context, symbol).map_err(|error| error.to_string())?;
    let target = runtime.context.weak_value(link);
    (target != ncl_sys::Word::NIL)
        .then_some(target)
        .ok_or_else(|| "weak target was cleared".to_owned())
}

fn restore_weak_target(
    runtime: &mut Runtime,
    weak_symbol: ncl_sys::Word,
    recovery_symbol: ncl_sys::Word,
) -> Result<ncl_sys::Word, String> {
    let target = weak_target(runtime, weak_symbol)?;
    ncl_object::set_symbol_value(&mut runtime.context, recovery_symbol, target)
        .map_err(|error| error.to_string())?;
    Ok(target)
}

fn read_recovered_cell_car(
    runtime: &mut Runtime,
    recovery_symbol: &str,
) -> Result<ncl_sys::Word, String> {
    let value = runtime
        .eval(&format!("(funcall (car {recovery_symbol}))"))
        .map_err(|error| error.to_string())?;
    ncl_object::car(&runtime.context, value).map_err(|error| error.to_string())
}
