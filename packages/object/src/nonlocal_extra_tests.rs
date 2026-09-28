#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::{Runtime, ThreadContext, Word};

    #[test]
    fn unwind_protect_restores_all_pending_values() {
        let runtime = Runtime::new().expect("runtime"); // check-added-lines: allow(panic)
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("register"); // check-added-lines: allow(panic)
        ctx.set_gc_stress(true);
        ctx.set_strict_forwarding(true);
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

    #[test]
    fn nested_cleanup_preserves_the_newest_pending_payload() {
        let runtime = Runtime::new().expect("runtime"); // check-added-lines: allow(panic)
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("register"); // check-added-lines: allow(panic)
        ctx.set_gc_stress(true);
        ctx.set_strict_forwarding(true);
        let outer_tag = Word::fixnum(10);
        let cleanup_tag = Word::fixnum(11);
        ctx.enter_catch(outer_tag);
        ctx.enter_catch(cleanup_tag);
        ctx.enter_unwind_protect(1);
        ctx.throw(outer_tag, Word::fixnum(1)).expect("outer throw");
        ctx.leave_unwind_protect(1).expect("save outer payload");
        ctx.throw(cleanup_tag, Word::fixnum(2))
            .expect("cleanup throw");
        ctx.leave_unwind_protect(1).expect("cleanup takes priority");
        assert!(ctx.thread.pending()); // check-added-lines: allow(panic)
        assert_eq!(ctx.thread.multiple_values()[0], cleanup_tag); // check-added-lines: allow(panic,index)
        assert_eq!(ctx.thread.multiple_values()[1], Word::fixnum(2)); // check-added-lines: allow(panic,index)
        ctx.leave_catch().expect("consume cleanup throw");
        assert!(!ctx.thread.pending()); // check-added-lines: allow(panic)
        ctx.leave_catch().expect("leave outer catch");
    }

    #[test]
    fn nested_cleanup_restores_the_outer_payload_after_inner_catch() {
        let runtime = Runtime::new().expect("runtime"); // check-added-lines: allow(panic)
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("register"); // check-added-lines: allow(panic)
        let outer_tag = Word::fixnum(20);
        let inner_tag = Word::fixnum(21);
        ctx.enter_catch(outer_tag);
        ctx.enter_unwind_protect(2);
        ctx.throw(outer_tag, Word::fixnum(3)).expect("outer throw");
        ctx.leave_unwind_protect(2).expect("save outer payload");
        ctx.enter_catch(inner_tag);
        ctx.throw(inner_tag, Word::fixnum(4)).expect("inner throw");
        ctx.leave_catch().expect("consume inner throw");
        ctx.leave_unwind_protect(2).expect("restore outer payload");
        assert!(ctx.thread.pending()); // check-added-lines: allow(panic)
        assert_eq!(ctx.thread.multiple_values()[0], outer_tag); // check-added-lines: allow(panic,index)
        assert_eq!(ctx.thread.multiple_values()[1], Word::fixnum(3)); // check-added-lines: allow(panic,index)
        ctx.leave_catch().expect("consume outer throw");
    }
}
