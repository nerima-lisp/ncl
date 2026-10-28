use super::*;

struct RemfBindings {
    place_temporary: Word,
    property_binding: Word,
    property_list: Word,
    found: Word,
    labels: Vec<Word>,
}

fn assert_remf_outer_bindings(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    expansion: &[Word],
    list: Word,
    property: Word,
) -> RemfBindings {
    assert_eq!(
        expansion[0],
        symbol(ctx, runtime, "LET*").unwrap_or_else(super::test_operation_failure)
    );
    let outer_bindings = elements(ctx, expansion[1]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(outer_bindings.len(), 3);
    let place_temporary =
        elements(ctx, outer_bindings[0]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(place_temporary[1], list);
    let property_binding =
        elements(ctx, outer_bindings[1]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(property_binding[1], property);
    let place_binding =
        elements(ctx, outer_bindings[2]).unwrap_or_else(super::test_operation_failure);
    let property_list = place_binding[0];
    assert_eq!(
        elements(ctx, place_binding[1]).unwrap_or_else(super::test_operation_failure),
        vec![
            symbol(ctx, runtime, "CAR").unwrap_or_else(super::test_operation_failure),
            place_temporary[0],
        ]
    );

    let found_let = elements(ctx, expansion[2]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(
        found_let[0],
        symbol(ctx, runtime, "LET").unwrap_or_else(super::test_operation_failure)
    );
    let found_bindings = elements(ctx, found_let[1]).unwrap_or_else(super::test_operation_failure);
    let found_binding =
        elements(ctx, found_bindings[0]).unwrap_or_else(super::test_operation_failure);
    let found = found_binding[0];
    assert_eq!(found_binding[1], Word::NIL);

    let labels = elements(ctx, found_let[2]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(
        labels[0],
        symbol(ctx, runtime, "LABELS").unwrap_or_else(super::test_operation_failure)
    );
    RemfBindings {
        place_temporary: place_temporary[0],
        property_binding: property_binding[0],
        property_list,
        found,
        labels,
    }
}

fn assert_remf_finder(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    labels: &[Word],
    property_binding: Word,
    found: Word,
) -> (Word, Word, Word) {
    let definitions = elements(ctx, labels[1]).unwrap_or_else(super::test_operation_failure);
    let definition = elements(ctx, definitions[0]).unwrap_or_else(super::test_operation_failure);
    let finder = definition[0];
    let parameters = elements(ctx, definition[1]).unwrap_or_else(super::test_operation_failure);
    let parameter = parameters[0];
    let finder_body = elements(ctx, definition[2]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(finder_body[2], Word::NIL);
    let choose = elements(ctx, finder_body[3]).unwrap_or_else(super::test_operation_failure);
    let match_test = elements(ctx, choose[1]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(
        elements(ctx, match_test[1]).unwrap_or_else(super::test_operation_failure),
        vec![
            symbol(ctx, runtime, "CAR").unwrap_or_else(super::test_operation_failure),
            parameter,
        ]
    );
    assert_eq!(match_test[2], property_binding);

    let matched = elements(ctx, choose[2]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(
        elements(ctx, matched[1]).unwrap_or_else(super::test_operation_failure),
        vec![
            symbol(ctx, runtime, "SETQ").unwrap_or_else(super::test_operation_failure),
            found,
            symbol(ctx, runtime, "T").unwrap_or_else(super::test_operation_failure),
        ]
    );
    assert_eq!(
        elements(ctx, matched[2]).unwrap_or_else(super::test_operation_failure),
        vec![
            symbol(ctx, runtime, "CDDR").unwrap_or_else(super::test_operation_failure),
            parameter,
        ]
    );
    (finder, parameter, choose[3])
}

fn assert_remf_recursive_splice(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    recursive: Word,
    finder: Word,
    parameter: Word,
    found: Word,
) {
    let recursive = elements(ctx, recursive).unwrap_or_else(super::test_operation_failure);
    let recursive_bindings =
        elements(ctx, recursive[1]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(recursive_bindings.len(), 2);
    let rest_binding =
        elements(ctx, recursive_bindings[0]).unwrap_or_else(super::test_operation_failure);
    let tail_binding =
        elements(ctx, recursive_bindings[1]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(
        elements(ctx, rest_binding[1]).unwrap_or_else(super::test_operation_failure),
        vec![
            symbol(ctx, runtime, "CDDR").unwrap_or_else(super::test_operation_failure),
            parameter,
        ]
    );
    assert_eq!(
        elements(ctx, tail_binding[1]).unwrap_or_else(super::test_operation_failure),
        vec![finder, rest_binding[0]]
    );
    let after_recursion = elements(ctx, recursive[2]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(after_recursion[1], found);
    let splice_and_return =
        elements(ctx, after_recursion[2]).unwrap_or_else(super::test_operation_failure);
    let previous_value =
        elements(ctx, splice_and_return[1]).unwrap_or_else(super::test_operation_failure);
    let previous_tail =
        elements(ctx, previous_value[1]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(
        previous_value[0],
        symbol(ctx, runtime, "RPLACD").unwrap_or_else(super::test_operation_failure)
    );
    assert_eq!(
        previous_tail,
        vec![
            symbol(ctx, runtime, "CDR").unwrap_or_else(super::test_operation_failure),
            parameter,
        ]
    );
    assert_eq!(previous_value[2], tail_binding[0]);
    assert_eq!(splice_and_return[2], parameter);
    assert_eq!(after_recursion[3], parameter);
}

fn assert_remf_result_store(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    remf_body: Word,
    property_list: Word,
    found: Word,
    finder: Word,
    place_temporary: Word,
) {
    let remf_body = elements(ctx, remf_body).unwrap_or_else(super::test_operation_failure);
    let result_binding = elements(ctx, remf_body[1]).unwrap_or_else(super::test_operation_failure);
    let new_plist_binding =
        elements(ctx, result_binding[0]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(
        elements(ctx, new_plist_binding[1]).unwrap_or_else(super::test_operation_failure),
        vec![finder, property_list]
    );
    let result = elements(ctx, remf_body[2]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(result[1], found);
    assert_eq!(result[3], Word::NIL);
    let success = elements(ctx, result[2]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(
        success[2],
        symbol(ctx, runtime, "T").unwrap_or_else(super::test_operation_failure)
    );
    let store = elements(ctx, success[1]).unwrap_or_else(super::test_operation_failure);
    let store_bindings = elements(ctx, store[1]).unwrap_or_else(super::test_operation_failure);
    let store_binding =
        elements(ctx, store_bindings[0]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(store_binding[1], new_plist_binding[0]);
    let store_body = elements(ctx, store[2]).unwrap_or_else(super::test_operation_failure);
    let rplaca = elements(ctx, store_body[1]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(rplaca[1], place_temporary);
    assert_eq!(rplaca[2], store_binding[0]);
    assert_eq!(store_body[2], store_binding[0]);
}

#[test]
fn remf_expansion_tracks_matches_splices_tail_and_returns_presence() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx).unwrap_or_else(super::test_operation_failure);
    let registry = PlaceRegistry::new(&runtime);
    let list = symbol(&mut ctx, &runtime, "LIST").unwrap_or_else(super::test_operation_failure);
    let place = place_form(&mut ctx, &runtime, "CAR", &[list])
        .unwrap_or_else(super::test_operation_failure);
    let property =
        symbol(&mut ctx, &runtime, "PROPERTY").unwrap_or_else(super::test_operation_failure);
    let expansion = expand_remf(&mut ctx, &runtime, &registry, &[place, property])
        .unwrap_or_else(super::test_operation_failure);

    let expansion = elements(&mut ctx, expansion).unwrap_or_else(super::test_operation_failure);
    let bindings = assert_remf_outer_bindings(&mut ctx, &runtime, &expansion, list, property);
    let (finder, parameter, recursive) = assert_remf_finder(
        &mut ctx,
        &runtime,
        &bindings.labels,
        bindings.property_binding,
        bindings.found,
    );
    assert_remf_recursive_splice(
        &mut ctx,
        &runtime,
        recursive,
        finder,
        parameter,
        bindings.found,
    );
    assert_remf_result_store(
        &mut ctx,
        &runtime,
        bindings.labels[2],
        bindings.property_list,
        bindings.found,
        finder,
        bindings.place_temporary,
    );
}
