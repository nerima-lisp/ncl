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
