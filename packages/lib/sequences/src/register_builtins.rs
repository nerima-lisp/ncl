use super::register_extra::{
    adjoin_entry, assoc_entry, every_entry, intersection_entry, map_entry, map_into_entry,
    mapc_entry, mapcan_entry, mapcar_entry, mapcon_entry, mapl_entry, maplist_entry, member_entry,
    notany_entry, notevery_entry, rassoc_entry, reduce_entry, remove_entry, set_difference_entry,
    set_exclusive_or_entry, some_entry, sort_entry, stable_sort_entry, subsetp_entry,
    substitute_entry, union_entry,
};
use super::{
    DESTINATION_TYPE, INDEX, LIST, ONE_OBJECT, REST, SEQUENCE, TWO_OBJECTS, append_builtin,
    atom_builtin, car_builtin, cdr_builtin, concatenate_builtin, cons_builtin, cons_p_builtin,
    copy_list_builtin, copy_seq_builtin, count_entry, direct_descriptor, domain, eighth_builtin,
    elt_builtin, endp_builtin, fifth_builtin, find_entry, first_builtin, fourth_builtin,
    length_builtin, list_builtin, list_length_builtin, list_p_builtin, list_star_builtin,
    mismatch_entry, nconc_builtin, ninth_builtin, nreverse_builtin, nth_builtin, nthcdr_builtin,
    position_entry, register_adapted, register_direct, reverse_builtin, rplaca_builtin,
    rplacd_builtin, search_entry, second_builtin, seventh_builtin, sixth_builtin, subseq_builtin,
    tenth_builtin, third_builtin,
};
use ncl_object::{
    Builtin, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, ObjectError, Runtime, ThreadContext,
};

#[allow(clippy::too_many_lines)]
/// Register the implemented list and sequence builtins.
///
/// # Errors
///
/// Returns an object error when a builtin cannot be registered.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for (name, function, required) in [
        ("ATOM", atom_builtin as ncl_object::RustBuiltin, ONE_OBJECT),
        ("CONSP", cons_p_builtin, ONE_OBJECT),
        ("LISTP", list_p_builtin, ONE_OBJECT),
        ("ENDP", endp_builtin, ONE_OBJECT),
        ("CAR", car_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CDR", cdr_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CONS", cons_builtin, TWO_OBJECTS),
        ("RPLACA", rplaca_builtin, TWO_OBJECTS),
        ("RPLACD", rplacd_builtin, TWO_OBJECTS),
        ("COPY-LIST", copy_list_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("NTH", nth_builtin, &[INDEX, LIST][..]),      // check-added-lines: allow(index) slice type
        ("NTHCDR", nthcdr_builtin, &[INDEX, LIST][..]), // check-added-lines: allow(index) slice type
        ("LIST-LENGTH", list_length_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("FIRST", first_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("SECOND", second_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("THIRD", third_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("FOURTH", fourth_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("FIFTH", fifth_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("SIXTH", sixth_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("SEVENTH", seventh_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("EIGHTH", eighth_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("NINTH", ninth_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("TENTH", tenth_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
    ] {
        runtime.register_builtin(
            ctx_ref(&mut ctx),
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(direct_descriptor(required), function),
        )?;
    }
    let rest = Builtin {
        lambda_list: LambdaList::with_rest(&[], REST),
        convention: BuiltinConvention::Adapted,
    };
    for (name, function) in [
        ("LIST", list_builtin as ncl_object::RustBuiltin),
        ("APPEND", append_builtin),
        ("NCONC", nconc_builtin),
    ] {
        register_adapted(runtime, &mut ctx, name, rest, function)?;
    }
    register_adapted(
        runtime,
        &mut ctx,
        "LIST*",
        Builtin {
            lambda_list: LambdaList::with_rest(ONE_OBJECT, REST),
            convention: BuiltinConvention::Adapted,
        },
        list_star_builtin,
    )?;
    register_adapted(
        runtime,
        &mut ctx,
        "CONCATENATE",
        Builtin {
            lambda_list: LambdaList::with_rest(&[DESTINATION_TYPE], REST),
            convention: BuiltinConvention::Adapted,
        },
        concatenate_builtin,
    )?;
    for (name, function, params) in [
        (
            "LENGTH",
            length_builtin as ncl_object::RustBuiltin,
            &[SEQUENCE][..], // check-added-lines: allow(index) slice type
        ),
        ("COPY-SEQ", copy_seq_builtin, &[SEQUENCE][..]), // check-added-lines: allow(index) slice type
        ("REVERSE", reverse_builtin, &[SEQUENCE][..]), // check-added-lines: allow(index) slice type
        ("NREVERSE", nreverse_builtin, &[SEQUENCE][..]), // check-added-lines: allow(index) slice type
        ("ELT", elt_builtin, &[SEQUENCE, INDEX][..]), // check-added-lines: allow(index) slice type
    ] {
        register_direct(runtime, &mut ctx, name, direct_descriptor(params), function)?;
    }
    register_adapted(
        runtime,
        &mut ctx,
        "SUBSEQ",
        Builtin {
            lambda_list: LambdaList::with_optional(&[SEQUENCE, INDEX], &[INDEX]),
            convention: BuiltinConvention::Adapted,
        },
        subseq_builtin,
    )?;
    for name in ["FILL", "REPLACE"] {
        // check-added-lines: allow(index) slice type
        let Some(implementation) = domain::filter::filter_entry(name) else {
            return Err(ObjectError::TypeError);
        };
        runtime.register_builtin(
            ctx_ref(&mut ctx),
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            implementation,
        )?;
    }
    let variadic = Builtin {
        lambda_list: LambdaList::with_rest(&[], REST),
        convention: BuiltinConvention::Adapted,
    };
    for (name, function) in [
        ("FIND", find_entry as ncl_object::RustBuiltin),
        ("POSITION", position_entry),
        ("COUNT", count_entry),
        ("SEARCH", search_entry),
        ("MISMATCH", mismatch_entry),
        ("MAPCAR", mapcar_entry),
        ("MAPC", mapc_entry),
        ("MAPLIST", maplist_entry),
        ("MAPL", mapl_entry),
        ("MAPCAN", mapcan_entry),
        ("MAPCON", mapcon_entry),
        ("MAP", map_entry),
        ("MAP-INTO", map_into_entry),
        ("REDUCE", reduce_entry),
        ("EVERY", every_entry),
        ("SOME", some_entry),
        ("NOTANY", notany_entry),
        ("NOTEVERY", notevery_entry),
        ("REMOVE", remove_entry),
        ("SUBSTITUTE", substitute_entry),
        ("SORT", sort_entry),
        ("STABLE-SORT", stable_sort_entry),
        ("UNION", union_entry),
        ("INTERSECTION", intersection_entry),
        ("SET-DIFFERENCE", set_difference_entry),
        ("SET-EXCLUSIVE-OR", set_exclusive_or_entry),
        ("SUBSETP", subsetp_entry),
        ("ADJOIN", adjoin_entry),
        ("ASSOC", assoc_entry),
        ("RASSOC", rassoc_entry),
        ("MEMBER", member_entry),
    ] {
        register_adapted(runtime, &mut ctx, name, variadic, function)?;
    }
    Ok(())
}
const fn ctx_ref(ctx: &mut ThreadContext) -> &mut ThreadContext {
    ctx
}
