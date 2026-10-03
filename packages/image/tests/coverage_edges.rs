#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    missing_docs,
    reason = "coverage cases assert on concrete image values"
)]

//! Table-driven coverage for package and uninterned-symbol image records.

use ncl_image::{load, save};
use ncl_object::{
    car, cdr, make_string, make_symbol, set_symbol_value, string_length, string_ref, symbol_name,
    symbol_package, symbol_value, Package, Runtime, ThreadContext, Word,
};

fn read_string(ctx: &ThreadContext, value: Word) -> String {
    (0..string_length(ctx, value).expect("string length"))
        .map(|index| string_ref(ctx, value, index).expect("string character"))
        .collect()
}

#[test]
fn uninterned_symbols_round_trip_with_value_cases() {
    struct Case {
        name: &'static str,
        value: i64,
    }

    for case in [
        Case {
            name: "IMAGE-UNINTERNED-A",
            value: 11,
        },
        Case {
            name: "IMAGE-UNINTERNED-B",
            value: -17,
        },
    ] {
        let runtime = Runtime::new().expect("source runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("source registration");
        let name = make_string(&mut ctx, &runtime, &case.name.chars().collect::<Vec<_>>())
            .expect("symbol name");
        let symbol = make_symbol(&mut ctx, &runtime, name).expect("uninterned symbol");
        set_symbol_value(&mut ctx, symbol, Word::fixnum(case.value)).expect("symbol value");

        let image = save(&runtime, &mut ctx, &[symbol], &[]).expect("save symbol");
        let destination = Runtime::new().expect("destination runtime");
        let mut destination_ctx = ThreadContext::new();
        destination_ctx
            .register(&destination)
            .expect("destination registration");
        let loaded = load(&image, &destination, &mut destination_ctx).expect("load symbol");
        let loaded_symbol = loaded.roots[0];

        assert_eq!(
            symbol_package(&destination_ctx, loaded_symbol),
            Ok(Word::NIL)
        );
        assert_eq!(
            read_string(
                &destination_ctx,
                symbol_name(&destination_ctx, loaded_symbol).expect("loaded symbol name"),
            ),
            case.name
        );
        assert_eq!(
            symbol_value(&destination_ctx, loaded_symbol),
            Ok(Word::fixnum(case.value))
        );
    }
}

#[test]
fn package_nickname_cases_round_trip_in_order() {
    struct Case {
        name: &'static str,
        nicknames: &'static [&'static str],
    }

    for case in [
        Case {
            name: "IMAGE-PACKAGE-ONE",
            nicknames: &["IMAGE-ONE"],
        },
        Case {
            name: "IMAGE-PACKAGE-TWO",
            nicknames: &["IMAGE-TWO", "IMAGE-SECOND"],
        },
    ] {
        let runtime = Runtime::new().expect("source runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("source registration");
        let package = Package::new(&mut ctx, &runtime, case.name).expect("package");
        for nickname in case.nicknames {
            let value = make_string(&mut ctx, &runtime, &nickname.chars().collect::<Vec<_>>())
                .expect("nickname");
            assert_eq!(
                package
                    .add_nickname(&mut ctx, &runtime, value)
                    .expect("add nickname"),
                true
            );
        }

        let image = save(&runtime, &mut ctx, &[package.as_word()], &[]).expect("save package");
        let destination = Runtime::new().expect("destination runtime");
        let mut destination_ctx = ThreadContext::new();
        destination_ctx
            .register(&destination)
            .expect("destination registration");
        let loaded = load(&image, &destination, &mut destination_ctx).expect("load package");
        let loaded_package = Package::from_word(loaded.roots[0]);
        assert_eq!(
            read_string(
                &destination_ctx,
                loaded_package.name(&destination_ctx).expect("package name"),
            ),
            case.name
        );

        let mut names = loaded_package
            .nicknames(&destination_ctx)
            .expect("package nicknames");
        for expected in case.nicknames {
            let nickname = car(&destination_ctx, names).expect("nickname list car");
            assert_eq!(read_string(&destination_ctx, nickname), *expected);
            names = cdr(&destination_ctx, names).expect("nickname list cdr");
        }
        assert_eq!(names, Word::NIL);
    }
}
