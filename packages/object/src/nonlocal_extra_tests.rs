#[cfg(test)]
mod tests {
    use crate::{Runtime, ThreadContext, Word};

    #[test]
    fn unwind_protect_restores_all_pending_values() {
        let runtime = Runtime::new().expect("runtime"); // check-added-lines: allow(panic)
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("register"); // check-added-lines: allow(panic)
        let tag = Word::fixnum(4);
        ctx.enter_catch(tag);
        ctx.enter_unwind_protect(2);
        ctx.thread
            .set_multiple_value_area(&[tag, Word::fixnum(6), Word::fixnum(7)]);
        ctx.thread.set_pending(true);
        ctx.leave_unwind_protect(2).expect("save values"); // check-added-lines: allow(panic)
        ctx.leave_unwind_protect(2).expect("restore values"); // check-added-lines: allow(panic)
        assert!(ctx.thread.pending()); // check-added-lines: allow(panic)
        assert_eq!(ctx.thread.mv_count(), 3); // check-added-lines: allow(panic)
        assert_eq!(ctx.thread.multiple_values()[1], Word::fixnum(6)); // check-added-lines: allow(panic,index)
        assert_eq!(ctx.thread.multiple_values()[2], Word::fixnum(7)); // check-added-lines: allow(panic,index)
        let _ = ctx.leave_catch();
    }
}
