#[cfg(test)]
mod behavior_tests {
    use super::super::{HashTable, HashTest, Weakness, sxhash};
    use crate::{Runtime, ThreadContext};
    use ncl_sys::Word;

    fn setup() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        (runtime, ctx)
    }

    #[test]
    fn resize_and_tombstone_reuse_preserve_value_pairs() {
        let (runtime, mut ctx) = setup();
        let table = HashTable::new(&mut ctx, &runtime, HashTest::Eql, Weakness::None)
            .unwrap_or_else(|error| panic!("table: {error:?}"));
        let colliding = (0..128_i64)
            .filter(|value| sxhash(Word::fixnum(*value)) & 7 == sxhash(Word::fixnum(0)) & 7)
            .take(4)
            .collect::<Vec<_>>();
        assert_eq!(colliding.len(), 4);

        for (index, key) in colliding.iter().enumerate() {
            table
                .insert(
                    &mut ctx,
                    &runtime,
                    Word::fixnum(*key),
                    Word::fixnum(i64::try_from(index).unwrap_or(i64::MAX)),
                )
                .unwrap_or_else(|error| panic!("insert: {error:?}"));
        }
        assert_eq!(table.capacity(&ctx), Ok(8));
        assert_eq!(
            table.remove(&mut ctx, &runtime, Word::fixnum(colliding[1])),
            Ok(Some(Word::fixnum(1)))
        );
        table
            .insert(&mut ctx, &runtime, Word::fixnum(99), Word::fixnum(99))
            .unwrap_or_else(|error| panic!("tombstone insert: {error:?}"));
        assert_eq!(
            table.get(&mut ctx, Word::fixnum(99)),
            Ok(Some(Word::fixnum(99)))
        );
        assert_eq!(
            table.get(&mut ctx, Word::fixnum(colliding[2])),
            Ok(Some(Word::fixnum(2)))
        );

        for key in 100..108_i64 {
            table
                .insert(
                    &mut ctx,
                    &runtime,
                    Word::fixnum(key),
                    Word::fixnum(key * 10),
                )
                .unwrap_or_else(|error| panic!("resize insert: {error:?}"));
        }
        assert_eq!(table.capacity(&ctx), Ok(16));
        let mut entries = Vec::new();
        table
            .for_each_entry(&ctx, |key, value| entries.push((key, value)))
            .unwrap_or_else(|error| panic!("entries: {error:?}"));
        entries.sort_by_key(|(key, _)| key.as_fixnum().unwrap_or(i64::MIN));
        assert!(
            entries
                .iter()
                .any(|(key, value)| *key == Word::fixnum(99) && *value == Word::fixnum(99))
        );
        assert!(
            !entries
                .iter()
                .any(|(key, _)| *key == Word::fixnum(colliding[1]))
        );
        assert_eq!(table.count(&ctx), Ok(12));
    }
}

#[cfg(test)]
mod direct_tests {
    use super::super::{HashTable, HashTest, Weakness};
    use crate::{Runtime, ThreadContext, make_string};
    use ncl_sys::Word;

    fn setup() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        (runtime, ctx)
    }

    #[test]
    fn lookup_replacement_removal_and_iteration_have_value_results() {
        let (runtime, mut ctx) = setup();
        let table = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
            .unwrap_or_else(|error| panic!("table: {error:?}"));
        let key = make_string(&mut ctx, &runtime, &['k']).unwrap_or(Word::NIL);
        table
            .insert(&mut ctx, &runtime, key, Word::fixnum(1))
            .unwrap_or_else(|error| panic!("insert: {error:?}"));
        assert_eq!(table.get(&mut ctx, key), Ok(Some(Word::fixnum(1))));
        table
            .insert(&mut ctx, &runtime, key, Word::fixnum(2))
            .unwrap_or_else(|error| panic!("replace: {error:?}"));
        assert_eq!(table.count(&ctx), Ok(1));
        assert_eq!(table.get(&mut ctx, key), Ok(Some(Word::fixnum(2))));
        assert_eq!(table.remove(&mut ctx, &runtime, Word::fixnum(99)), Ok(None));
        assert_eq!(
            table.remove(&mut ctx, &runtime, key),
            Ok(Some(Word::fixnum(2)))
        );
        assert_eq!(table.count(&ctx), Ok(0));
        let mut entries = Vec::new();
        table
            .for_each_entry(&ctx, |stored_key, value| entries.push((stored_key, value)))
            .unwrap_or_else(|error| panic!("entries: {error:?}"));
        assert!(entries.is_empty());
    }

    #[test]
    fn threshold_options_drive_additive_and_multiplicative_resize() {
        let (runtime, mut ctx) = setup();
        let additive = HashTable::new_with_options(
            &mut ctx,
            &runtime,
            HashTest::Eql,
            Weakness::None,
            Word::fixnum(1),
            Some(Word::fixnum(3)),
            Some(Word::fixnum(0)),
        );
        assert!(additive.is_err());
        let additive = HashTable::new_with_options(
            &mut ctx,
            &runtime,
            HashTest::Eql,
            Weakness::None,
            Word::fixnum(1),
            Some(Word::fixnum(3)),
            Some(Word::fixnum(1)),
        )
        .unwrap_or_else(|error| panic!("additive table: {error:?}"));
        for value in 0..9_i64 {
            additive
                .insert(
                    &mut ctx,
                    &runtime,
                    Word::fixnum(value),
                    Word::fixnum(value * 2),
                )
                .unwrap_or_else(|error| panic!("additive insert {value}: {error:?}"));
        }
        assert!(additive.capacity(&ctx).unwrap_or(0) >= 16);
        assert_eq!(
            additive.get(&mut ctx, Word::fixnum(8)),
            Ok(Some(Word::fixnum(16)))
        );
        let factor = crate::make_double(&mut ctx, &runtime, 2.0)
            .unwrap_or_else(|error| panic!("factor: {error:?}"));
        let multiplicative = HashTable::new_with_options(
            &mut ctx,
            &runtime,
            HashTest::Equal,
            Weakness::None,
            Word::fixnum(8),
            Some(factor.into()),
            Some(Word::fixnum(1)),
        )
        .unwrap_or_else(|error| panic!("multiplicative table: {error:?}"));
        for value in 0..9_i64 {
            multiplicative
                .insert(&mut ctx, &runtime, Word::fixnum(value), Word::fixnum(value))
                .unwrap_or_else(|error| panic!("multiplicative insert: {error:?}"));
        }
        assert_eq!(multiplicative.capacity(&ctx), Ok(16));
    }
}
