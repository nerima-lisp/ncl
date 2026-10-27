use super::{
    PlaceRegistry, Result, RootedExpansion, add_place_bindings, binding, form, fresh_symbol, place,
    place_value, store_place_prebound, wrap_bindings, wrap_let,
};
use crate::form::list;
use ncl_object::{HeapRootedWord, ObjectError, Runtime, ThreadContext, Word};

fn rooted_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    args: &[Word],
) -> Result<HeapRootedWord> {
    let form = form(ctx, runtime, name, args)?;
    HeapRootedWord::new(ctx, form)
}

fn rooted_call(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    operator: Word,
    args: &[Word],
) -> Result<HeapRootedWord> {
    let mut values = Vec::with_capacity(args.len() + 1);
    values.push(operator);
    values.extend_from_slice(args);
    let call = list(ctx, runtime, &values)?;
    HeapRootedWord::new(ctx, call)
}

fn rooted_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<HeapRootedWord> {
    let list = list(ctx, runtime, values)?;
    HeapRootedWord::new(ctx, list)
}

fn fresh_root(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<HeapRootedWord> {
    let symbol = fresh_symbol(ctx, runtime)?;
    HeapRootedWord::new(ctx, symbol)
}

fn remf_finder_definition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    property_variable: &HeapRootedWord,
) -> Result<(
    HeapRootedWord,
    HeapRootedWord,
    HeapRootedWord,
    HeapRootedWord,
    HeapRootedWord,
)> {
    let finder = fresh_root(ctx, runtime)?;
    let parameter = fresh_root(ctx, runtime)?;
    let found = fresh_root(ctx, runtime)?;
    let rest = fresh_root(ctx, runtime)?;
    let new_tail = fresh_root(ctx, runtime)?;
    let new_plist = fresh_root(ctx, runtime)?;

    let empty = rooted_form(ctx, runtime, "ENDP", &[parameter.get()])?;
    let key = rooted_form(ctx, runtime, "CAR", &[parameter.get()])?;
    let matches = rooted_form(ctx, runtime, "EQ", &[key.get(), property_variable.get()])?;
    let true_symbol = crate::form::symbol(ctx, runtime, "T")?;
    let true_symbol = HeapRootedWord::new(ctx, true_symbol)?;
    let mark_found = rooted_form(ctx, runtime, "SETQ", &[found.get(), true_symbol.get()])?;
    let after_pair = rooted_form(ctx, runtime, "CDDR", &[parameter.get()])?;
    let remove_pair_branch =
        rooted_form(ctx, runtime, "PROGN", &[mark_found.get(), after_pair.get()])?;

    let recursive_rest = rooted_form(ctx, runtime, "CDDR", &[parameter.get()])?;
    let rest_binding_form = binding(ctx, runtime, rest.get(), recursive_rest.get())?;
    let rest_binding = HeapRootedWord::new(ctx, rest_binding_form)?;
    let recursive_call = rooted_call(ctx, runtime, finder.get(), &[rest.get()])?;
    let tail_binding_form = binding(ctx, runtime, new_tail.get(), recursive_call.get())?;
    let tail_binding = HeapRootedWord::new(ctx, tail_binding_form)?;
    let tail_bindings = rooted_list(ctx, runtime, &[rest_binding.get(), tail_binding.get()])?;
    let previous_value = rooted_form(ctx, runtime, "CDR", &[parameter.get()])?;
    let splice = rooted_form(
        ctx,
        runtime,
        "RPLACD",
        &[previous_value.get(), new_tail.get()],
    )?;
    let splice_and_return = rooted_form(ctx, runtime, "PROGN", &[splice.get(), parameter.get()])?;
    let after_recursion = rooted_form(
        ctx,
        runtime,
        "IF",
        &[found.get(), splice_and_return.get(), parameter.get()],
    )?;
    let recursive_branch = rooted_form(
        ctx,
        runtime,
        "LET*",
        &[tail_bindings.get(), after_recursion.get()],
    )?;
    let choose_branch = rooted_form(
        ctx,
        runtime,
        "IF",
        &[
            matches.get(),
            remove_pair_branch.get(),
            recursive_branch.get(),
        ],
    )?;
    let function_body = rooted_form(
        ctx,
        runtime,
        "IF",
        &[empty.get(), Word::NIL, choose_branch.get()],
    )?;
    let parameters = rooted_list(ctx, runtime, &[parameter.get()])?;
    let definition = rooted_list(
        ctx,
        runtime,
        &[finder.get(), parameters.get(), function_body.get()],
    )?;
    let definitions = rooted_list(ctx, runtime, &[definition.get()])?;

    Ok((finder, found, new_plist, definitions, true_symbol))
}

pub fn expand_remf(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
) -> Result {
    if arguments.len() != 2 {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_heap_rooted_slice(ctx, arguments, |ctx, arguments| {
        let place_word = arguments.first().ok_or(ObjectError::TypeError)?;
        let property = arguments.get(1).ok_or(ObjectError::TypeError)?;
        let expansion = place(ctx, runtime, registry, place_word.get())?;
        let expansion = RootedExpansion::new(ctx, expansion)?;

        let mut bindings = Vec::new();
        add_place_bindings(ctx, runtime, &expansion, &mut bindings)?;
        let property_variable = fresh_symbol(ctx, runtime)?;
        let property_variable = HeapRootedWord::new(ctx, property_variable)?;
        let property_binding = binding(ctx, runtime, property_variable.get(), property.get())?;
        bindings.push(HeapRootedWord::new(ctx, property_binding)?);

        let property_list = fresh_symbol(ctx, runtime)?;
        let property_list = HeapRootedWord::new(ctx, property_list)?;
        let list_binding = binding(
            ctx,
            runtime,
            property_list.get(),
            expansion.access_form.get(),
        )?;
        bindings.push(HeapRootedWord::new(ctx, list_binding)?);

        let (finder, found, new_plist, definitions, true_symbol) =
            remf_finder_definition(ctx, runtime, &property_variable)?;
        let find_call = rooted_call(ctx, runtime, finder.get(), &[property_list.get()])?;
        let plist_binding_form = binding(ctx, runtime, new_plist.get(), find_call.get())?;
        let plist_binding = HeapRootedWord::new(ctx, plist_binding_form)?;
        let plist_bindings = rooted_list(ctx, runtime, &[plist_binding.get()])?;
        let value = place_value(ctx, runtime, place_word.get(), new_plist.get())?;
        let value = HeapRootedWord::new(ctx, value)?;
        let store = store_place_prebound(ctx, runtime, &expansion.current(), value.get())?;
        let store = HeapRootedWord::new(ctx, store)?;
        let success = rooted_form(ctx, runtime, "PROGN", &[store.get(), true_symbol.get()])?;
        let result = rooted_form(ctx, runtime, "IF", &[found.get(), success.get(), Word::NIL])?;
        let result = rooted_form(ctx, runtime, "LET*", &[plist_bindings.get(), result.get()])?;
        let labels = rooted_form(ctx, runtime, "LABELS", &[definitions.get(), result.get()])?;
        let found_binding_form = binding(ctx, runtime, found.get(), Word::NIL)?;
        let found_binding = HeapRootedWord::new(ctx, found_binding_form)?;
        let body = wrap_let(ctx, runtime, "LET", &[found_binding.get()], labels.get())?;
        wrap_bindings(ctx, runtime, &bindings, body)
    })
}
