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
    BuiltinPackage, FromLispArg, LambdaList, LispError, List, ObjectError, Runtime, ThreadContext,
};

macro_rules! composed_list_accessor {
    ($name:ident, $($accessor:path),+ $(,)?) => {
        fn $name(ctx: &ThreadContext, runtime: &Runtime, mut value: List) -> Result<ncl_object::Word, LispError> {
            composed_list_accessor!(@apply ctx, runtime, value, $($accessor),+)
        }
    };
    (@apply $ctx:ident, $runtime:ident, $value:ident, $accessor:path) => {
        $accessor($ctx, $runtime, $value)
    };
    (@apply $ctx:ident, $runtime:ident, $value:ident, $accessor:path, $($rest:path),+ $(,)?) => {{
        let next = $accessor($ctx, $runtime, $value)?;
        $value = List::from_lisp_arg($ctx, next)?;
        composed_list_accessor!(@apply $ctx, $runtime, $value, $($rest),+)
    }};
}

macro_rules! define_composed_builtin {
    ($name:ident, $implementation:ident, $($accessor:path),+ $(,)?) => {
        composed_list_accessor!($implementation, $($accessor),+);
        ncl_object::typed_builtin!($name, $implementation, (value: List));
    };
}

define_composed_builtin!(
    caar_builtin,
    caar_implementation,
    domain::list::car,
    domain::list::car
);
fn cadr_implementation(
    ctx: &ThreadContext,
    runtime: &Runtime,
    value: List,
) -> Result<ncl_object::Word, LispError> {
    let value = domain::list::cdr(ctx, runtime, value)?;
    let value = List::from_lisp_arg(ctx, value)?;
    domain::list::car(ctx, runtime, value)
}
ncl_object::typed_builtin!(cadr_builtin, cadr_implementation, (value: List));
define_composed_builtin!(
    cdar_builtin,
    cdar_implementation,
    domain::list::car,
    domain::list::cdr
);
define_composed_builtin!(
    cddr_builtin,
    cddr_implementation,
    domain::list::cdr,
    domain::list::cdr
);
define_composed_builtin!(
    caaar_builtin,
    caaar_implementation,
    domain::list::car,
    domain::list::car,
    domain::list::car
);
define_composed_builtin!(
    caadr_builtin,
    caadr_implementation,
    domain::list::cdr,
    domain::list::car,
    domain::list::car
);
define_composed_builtin!(
    cadar_builtin,
    cadar_implementation,
    domain::list::car,
    domain::list::cdr,
    domain::list::car
);
define_composed_builtin!(
    caddr_builtin,
    caddr_implementation,
    domain::list::cdr,
    domain::list::cdr,
    domain::list::car
);
define_composed_builtin!(
    cdaar_builtin,
    cdaar_implementation,
    domain::list::car,
    domain::list::car,
    domain::list::cdr
);
define_composed_builtin!(
    cdadr_builtin,
    cdadr_implementation,
    domain::list::cdr,
    domain::list::car,
    domain::list::cdr
);
define_composed_builtin!(
    cddar_builtin,
    cddar_implementation,
    domain::list::car,
    domain::list::cdr,
    domain::list::cdr
);
define_composed_builtin!(
    cdddr_builtin,
    cdddr_implementation,
    domain::list::cdr,
    domain::list::cdr,
    domain::list::cdr
);
define_composed_builtin!(
    caaaar_builtin,
    caaaar_implementation,
    domain::list::car,
    domain::list::car,
    domain::list::car,
    domain::list::car
);
define_composed_builtin!(
    caaadr_builtin,
    caaadr_implementation,
    domain::list::cdr,
    domain::list::car,
    domain::list::car,
    domain::list::car
);
define_composed_builtin!(
    caadar_builtin,
    caadar_implementation,
    domain::list::car,
    domain::list::cdr,
    domain::list::car,
    domain::list::car
);
define_composed_builtin!(
    caaddr_builtin,
    caaddr_implementation,
    domain::list::cdr,
    domain::list::cdr,
    domain::list::car,
    domain::list::car
);
define_composed_builtin!(
    cadaar_builtin,
    cadaar_implementation,
    domain::list::car,
    domain::list::car,
    domain::list::cdr,
    domain::list::car
);
define_composed_builtin!(
    cadadr_builtin,
    cadadr_implementation,
    domain::list::cdr,
    domain::list::car,
    domain::list::cdr,
    domain::list::car
);
define_composed_builtin!(
    caddar_builtin,
    caddar_implementation,
    domain::list::car,
    domain::list::cdr,
    domain::list::cdr,
    domain::list::car
);
define_composed_builtin!(
    cadddr_builtin,
    cadddr_implementation,
    domain::list::cdr,
    domain::list::cdr,
    domain::list::cdr,
    domain::list::car
);
define_composed_builtin!(
    cdaaar_builtin,
    cdaaar_implementation,
    domain::list::car,
    domain::list::car,
    domain::list::car,
    domain::list::cdr
);
define_composed_builtin!(
    cdaadr_builtin,
    cdaadr_implementation,
    domain::list::cdr,
    domain::list::car,
    domain::list::car,
    domain::list::cdr
);
define_composed_builtin!(
    cdadar_builtin,
    cdadar_implementation,
    domain::list::car,
    domain::list::cdr,
    domain::list::car,
    domain::list::cdr
);
define_composed_builtin!(
    cdaddr_builtin,
    cdaddr_implementation,
    domain::list::cdr,
    domain::list::cdr,
    domain::list::car,
    domain::list::cdr
);
define_composed_builtin!(
    cddaar_builtin,
    cddaar_implementation,
    domain::list::car,
    domain::list::car,
    domain::list::cdr,
    domain::list::cdr
);
define_composed_builtin!(
    cddadr_builtin,
    cddadr_implementation,
    domain::list::cdr,
    domain::list::car,
    domain::list::cdr,
    domain::list::cdr
);
define_composed_builtin!(
    cdddar_builtin,
    cdddar_implementation,
    domain::list::car,
    domain::list::cdr,
    domain::list::cdr,
    domain::list::cdr
);
define_composed_builtin!(
    cddddr_builtin,
    cddddr_implementation,
    domain::list::cdr,
    domain::list::cdr,
    domain::list::cdr,
    domain::list::cdr
);

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
        ("CAAR", caar_builtin, &[LIST][..]),   // check-added-lines: allow(index) slice type
        ("CADR", cadr_builtin, &[LIST][..]),   // check-added-lines: allow(index) slice type
        ("CDAR", cdar_builtin, &[LIST][..]),   // check-added-lines: allow(index) slice type
        ("CDDR", cddr_builtin, &[LIST][..]),   // check-added-lines: allow(index) slice type
        ("CAAAR", caaar_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CAADR", caadr_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CADAR", cadar_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CADDR", caddr_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CDAAR", cdaar_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CDADR", cdadr_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CDDAR", cddar_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CDDDR", cdddr_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CAAAAR", caaaar_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CAAADR", caaadr_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CAADAR", caadar_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CAADDR", caaddr_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CADAAR", cadaar_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CADADR", cadadr_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CADDAR", caddar_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CADDDR", cadddr_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CDAAAR", cdaaar_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CDAADR", cdaadr_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CDADAR", cdadar_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CDADDR", cdaddr_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CDDAAR", cddaar_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CDDADR", cddadr_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CDDDAR", cdddar_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
        ("CDDDDR", cddddr_builtin, &[LIST][..]), // check-added-lines: allow(index) slice type
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
    for name in std::iter::once("FILL").chain(std::iter::once("REPLACE")) {
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
