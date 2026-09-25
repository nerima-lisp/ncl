//! Builtin registration for lists, sequences, and trees.

use ncl_object::{
    Builtin, BuiltinImplementation, MultipleValues, ObjectError, Runtime, ThreadContext, Word, car,
    cdr, simple_vector_length, simple_vector_ref, string_length, string_ref,
};

const CL: &str = "COMMON-LISP";

fn direct(
    arity: u8,
    lambda_list: &'static str,
    function: ncl_object::RustBuiltin,
) -> BuiltinImplementation {
    BuiltinImplementation::direct(
        Builtin {
            arity,
            direct: true,
            lambda_list,
        },
        function,
    )
}

fn unsupported(
    _ctx: &mut ThreadContext,
    _args: &[Word],
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Err(ObjectError::Unsupported)
}

fn passthrough(args: &[Word]) -> Result<Vec<Word>, ObjectError> {
    Ok(args.to_vec())
}

fn two(args: &[Word]) -> Result<(Word, Word), ObjectError> {
    match args {
        [left, right] => Ok((*left, *right)),
        _ => Err(ObjectError::TypeError),
    }
}

fn cons_p(
    _ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if args.len() == 1 && args[0].is_cons() {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn atom(
    _ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if args.len() == 1 && !args[0].is_cons() {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn list_p(
    _ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    let mut cursor = args[0];
    while cursor.is_cons() {
        cursor = cdr(_ctx, cursor)?;
    }
    Ok(if cursor == Word::NIL {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn end_p(
    _ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    if args[0] == Word::NIL {
        return Ok(Word::TRUE);
    }
    if args[0].is_cons() {
        return Ok(Word::NIL);
    }
    Err(ObjectError::TypeError)
}

fn list_len(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    let mut slow = args[0];
    let mut fast = args[0];
    let mut length = 0_i64;
    loop {
        if fast == Word::NIL {
            return Ok(Word::fixnum(length));
        }
        if !fast.is_cons() {
            return Err(ObjectError::TypeError);
        }
        fast = cdr(ctx, fast)?;
        length += 1;
        if fast == Word::NIL {
            return Ok(Word::fixnum(length));
        }
        if !fast.is_cons() {
            return Err(ObjectError::TypeError);
        }
        fast = cdr(ctx, fast)?;
        slow = if slow.is_cons() {
            cdr(ctx, slow)?
        } else {
            Word::NIL
        };
        if fast == slow {
            return Ok(Word::NIL);
        }
        length += 1;
    }
}

fn nth(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (index, mut cursor) = two(args)?;
    let mut n = index.as_fixnum().ok_or(ObjectError::TypeError)?;
    if n < 0 {
        return Err(ObjectError::TypeError);
    }
    while n > 0 {
        if cursor == Word::NIL {
            return Ok(Word::NIL);
        }
        cursor = cdr(ctx, cursor)?;
        n -= 1;
    }
    if cursor == Word::NIL {
        Ok(Word::NIL)
    } else {
        car(ctx, cursor)
    }
}

fn nthcdr(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (index, mut cursor) = two(args)?;
    let mut n = index.as_fixnum().ok_or(ObjectError::TypeError)?;
    if n < 0 {
        return Err(ObjectError::TypeError);
    }
    while n > 0 {
        if cursor == Word::NIL {
            return Ok(Word::NIL);
        }
        cursor = cdr(ctx, cursor)?;
        n -= 1;
    }
    Ok(cursor)
}

fn length(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    if args[0] == Word::NIL || args[0].is_cons() {
        return list_len(ctx, args, &mut MultipleValues::new());
    }
    if let Ok(value) = simple_vector_length(ctx, args[0]) {
        return Ok(Word::fixnum(value as i64));
    }
    if let Ok(value) = string_length(ctx, args[0]) {
        return Ok(Word::fixnum(value as i64));
    }
    Err(ObjectError::TypeError)
}

fn elt(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (sequence, index) = two(args)?;
    let index = usize::try_from(index.as_fixnum().ok_or(ObjectError::TypeError)?)
        .map_err(|_| ObjectError::TypeError)?;
    if sequence == Word::NIL || sequence.is_cons() {
        return nth(
            ctx,
            &[Word::fixnum(index as i64), sequence],
            &mut MultipleValues::new(),
        );
    }
    if let Ok(value) = simple_vector_ref(ctx, sequence, index) {
        return Ok(value);
    }
    if let Ok(value) = string_ref(ctx, sequence, index) {
        return Ok(Word::character(value as u32));
    }
    Err(ObjectError::TypeError)
}

fn member(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (item, mut cursor) = two(args)?;
    while cursor != Word::NIL {
        if !cursor.is_cons() {
            return Err(ObjectError::TypeError);
        }
        if car(ctx, cursor)? == item {
            return Ok(cursor);
        }
        cursor = cdr(ctx, cursor)?;
    }
    Ok(Word::NIL)
}

fn assoc(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (item, mut cursor) = two(args)?;
    while cursor != Word::NIL {
        if !cursor.is_cons() {
            return Err(ObjectError::TypeError);
        }
        let entry = car(ctx, cursor)?;
        if entry.is_cons() && car(ctx, entry)? == item {
            return Ok(entry);
        }
        cursor = cdr(ctx, cursor)?;
    }
    Ok(Word::NIL)
}

fn car_builtin(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() == 1 {
        car(ctx, args[0])
    } else {
        Err(ObjectError::TypeError)
    }
}
fn cdr_builtin(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() == 1 {
        cdr(ctx, args[0])
    } else {
        Err(ObjectError::TypeError)
    }
}

macro_rules! positional {
    ($name:ident, $index:expr) => {
        fn $name(
            ctx: &mut ThreadContext,
            args: &[Word],
            values: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            if args.len() != 1 {
                return Err(ObjectError::TypeError);
            }
            nth(ctx, &[Word::fixnum($index), args[0]], values)
        }
    };
}
positional!(first, 0);
positional!(second, 1);
positional!(third, 2);
positional!(fourth, 3);
positional!(fifth, 4);
positional!(sixth, 5);
positional!(seventh, 6);
positional!(eighth, 7);
positional!(ninth, 8);
positional!(tenth, 9);

/// Register sequence builtins. Unsupported ownership rows are still interned and callable,
/// so later lanes can replace their implementations without changing registration shape.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    let implementations: &[(&str, BuiltinImplementation)] = &[
        ("ATOM", direct(1, "object", atom)),
        ("CONSP", direct(1, "object", cons_p)),
        ("LISTP", direct(1, "object", list_p)),
        ("ENDP", direct(1, "object", end_p)),
        ("LIST-LENGTH", direct(1, "list", list_len)),
        ("NTH", direct(2, "n list", nth)),
        ("NTHCDR", direct(2, "n list", nthcdr)),
        ("CAR", direct(1, "cons", car_builtin)),
        ("CDR", direct(1, "cons", cdr_builtin)),
        ("LENGTH", direct(1, "sequence", length)),
        ("ELT", direct(2, "sequence index", elt)),
        ("MEMBER", direct(2, "item list", member)),
        ("ASSOC", direct(2, "item alist", assoc)),
    ];
    for (name, implementation) in implementations {
        runtime.register_builtin(&mut ctx, CL, name, *implementation)?;
    }
    for (name, implementation) in [
        ("FIRST", first as ncl_object::RustBuiltin),
        ("SECOND", second),
        ("THIRD", third),
        ("FOURTH", fourth),
        ("FIFTH", fifth),
        ("SIXTH", sixth),
        ("SEVENTH", seventh),
        ("EIGHTH", eighth),
        ("NINTH", ninth),
        ("TENTH", tenth),
    ] {
        runtime.register_builtin(&mut ctx, CL, name, direct(1, "list", implementation))?;
    }
    for line in include_str!("../ownership.tsv").lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() >= 3 && fields[0] == CL && fields[2].contains("function") {
            let name = fields[1];
            if implementations.iter().any(|(known, _)| *known == name)
                || matches!(
                    name,
                    "FIRST"
                        | "SECOND"
                        | "THIRD"
                        | "FOURTH"
                        | "FIFTH"
                        | "SIXTH"
                        | "SEVENTH"
                        | "EIGHTH"
                        | "NINTH"
                        | "TENTH"
                )
            {
                continue;
            }
            runtime.register_builtin(
                &mut ctx,
                CL,
                name,
                BuiltinImplementation::adapted(
                    Builtin {
                        arity: 0,
                        direct: false,
                        lambda_list: "&rest arguments",
                    },
                    unsupported,
                    passthrough,
                ),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ncl_object::{FunctionObject, make_cons};

    fn setup() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("thread");
        register(&runtime).expect("sequence registration");
        (runtime, ctx)
    }

    fn list(ctx: &mut ThreadContext, runtime: &Runtime) -> Word {
        let tail = make_cons(ctx, runtime, Word::fixnum(3), Word::NIL).expect("cons");
        let middle = make_cons(ctx, runtime, Word::fixnum(2), tail).expect("cons");
        make_cons(ctx, runtime, Word::fixnum(1), middle).expect("cons")
    }

    fn call(
        runtime: &Runtime,
        ctx: &mut ThreadContext,
        name: &str,
        args: &[Word],
    ) -> Result<Word, ObjectError> {
        let function = runtime.function(ctx, CL, name).expect("function");
        runtime.call_builtin(ctx, FunctionObject::from(function), args)
    }

    #[test]
    fn basic_list_builtins_use_the_builtin_boundary() {
        let (runtime, mut ctx) = setup();
        let values = list(&mut ctx, &runtime);
        assert_eq!(
            call(&runtime, &mut ctx, "CAR", &[values]),
            Ok(Word::fixnum(1))
        );
        assert_eq!(
            call(&runtime, &mut ctx, "NTH", &[Word::fixnum(2), values]),
            Ok(Word::fixnum(3))
        );
        assert_eq!(
            call(&runtime, &mut ctx, "LENGTH", &[values]),
            Ok(Word::fixnum(3))
        );
        assert_eq!(
            call(&runtime, &mut ctx, "MEMBER", &[Word::fixnum(2), values]),
            Ok(ncl_object::cdr(&mut ctx, values).expect("cdr"))
        );
    }

    #[test]
    fn list_operations_reject_dotted_lists() {
        let (runtime, mut ctx) = setup();
        let dotted = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).expect("cons");
        assert_eq!(call(&runtime, &mut ctx, "LISTP", &[dotted]), Ok(Word::NIL));
        assert_eq!(
            call(&runtime, &mut ctx, "LENGTH", &[dotted]),
            Err(ObjectError::TypeError)
        );
    }
}
