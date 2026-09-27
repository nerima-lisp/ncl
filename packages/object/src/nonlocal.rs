//! GC-safe dynamic-extent frame chain for `catch`/`throw`, `unwind-protect`,
//! and `progv`.
//!
//! # Design
//!
//! Every native call boundary (an ordinary function/closure/builtin call
//! compiled by `ncl-codegen`) checks [`ncl_sys::Thread::pending`] after the
//! call returns. When it is set, the callee is propagating a non-local exit
//! rather than returning normally, and the caller must either intercept it
//! (because one of its own currently-open `catch`/`unwind-protect`/`progv`
//! regions is the target) or keep propagating by returning immediately
//! itself. This is a return-code propagation design (no raw stack/register
//! surgery): every native frame that is unwound past returns through its
//! ordinary epilogue, so frame bookkeeping the GC relies on elsewhere is
//! never disturbed. See the top-level README for the full design writeup.
//!
//! The payload of a propagating exit (the `catch` tag plus the thrown
//! value(s)) travels through the existing multiple-value return area
//! (`ncl_sys::Thread::mv`/`mv_count`), with the tag in word 0 and the
//! value(s) following. `unwind-protect` and `progv` frames intercept every
//! pass-through unconditionally (to run cleanup forms or restore bindings);
//! `catch` frames intercept only when their tag (compared with `eq`,
//! i.e. [`Word`] equality) matches word 0 of the propagating payload.
//!
//! Every frame's rooted payload is boxed so its address is stable
//! independent of this `Vec` reallocating, and is registered with the GC
//! root stack via [`ncl_sys::Thread::push_root_cell`] for exactly as long as
//! the frame is on the chain.

use std::cell::Cell;

use ncl_sys::{ControlFrameKind, RootToken};

use crate::{ObjectError, ThreadContext, Word, car, cdr, set_symbol_value, symbol_value};

/// A single rooted ABI word with a stable heap address.
///
/// Release is explicit (via [`Self::release`]) rather than through `Drop`,
/// because popping the root token needs `&mut ThreadContext`, which `Drop`
/// cannot borrow.
#[derive(Debug)]
pub struct RootedWord {
    slot: Box<Cell<Word>>,
    token: RootToken,
}

#[derive(Debug)]
pub struct PendingExit {
    region: u64,
    values: Vec<RootedWord>,
}

impl RootedWord {
    fn new(ctx: &mut ThreadContext, value: Word) -> Self {
        let slot = Box::new(Cell::new(value));
        let token = ctx.thread.push_root_cell(&slot);
        Self { slot, token }
    }

    fn get(&self) -> Word {
        self.slot.get()
    }

    fn release(self, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
        if ncl_sys::pop_root(&mut ctx.thread, self.token) {
            Ok(())
        } else {
            Err(ObjectError::RootStackCorrupted)
        }
    }
}

/// One `progv` binding: the special variable symbol and the value it held
/// before this frame rebound it.
#[derive(Debug)]
pub struct ProgvBinding {
    symbol: RootedWord,
    previous: RootedWord,
}

/// One entry in the dynamic-extent frame chain.
#[derive(Debug)]
pub enum DynamicFrame {
    /// A `catch` frame, matched by tag identity.
    Catch { tag: RootedWord },
    /// An `unwind-protect` frame; always runs its cleanup when unwound past.
    UnwindProtect { region: u64 },
    /// A `progv` frame; always restores its bindings when unwound past.
    Progv { bindings: Vec<ProgvBinding> },
}

fn restore_progv_bindings(
    ctx: &mut ThreadContext,
    bindings: Vec<ProgvBinding>,
) -> Result<(), ObjectError> {
    let mut result = Ok(());
    for binding in bindings.into_iter().rev() {
        let symbol = binding.symbol.get();
        let previous = binding.previous.get();
        if result.is_ok()
            && let Err(error) = set_symbol_value(ctx, symbol, previous)
        {
            result = Err(error);
        }
        if let Err(error) = binding.previous.release(ctx) {
            result = Err(error);
        }
        if let Err(error) = binding.symbol.release(ctx) {
            result = Err(error);
        }
    }
    result
}

impl DynamicFrame {
    const fn kind(&self) -> ControlFrameKind {
        match self {
            Self::Catch { .. } => ControlFrameKind::Catch,
            Self::UnwindProtect { .. } => ControlFrameKind::UnwindProtect,
            Self::Progv { .. } => ControlFrameKind::Progv,
        }
    }
}

impl ThreadContext {
    fn discard_pending_unwind(&mut self) -> Result<(), ObjectError> {
        let mut result = Ok(());
        while let Some(PendingExit { values, .. }) = self.pending_unwind.pop() {
            for value in values.into_iter().rev() {
                if let Err(error) = value.release(self) {
                    result = Err(error);
                }
            }
        }
        result
    }

    /// Establish a `catch` frame for `tag`.
    pub fn enter_catch(&mut self, tag: Word) {
        let frame = DynamicFrame::Catch {
            tag: RootedWord::new(self, tag),
        };
        self.thread.push_control_depth(frame.kind());
        self.frames.push(frame);
    }

    /// Leave the innermost `catch` frame.
    ///
    /// A mismatched inner catch is released while a throw is still
    /// propagating. Only the catch whose tag matches the pending payload may
    /// consume that pending exit.
    ///
    /// # Errors
    /// Returns [`ObjectError::RootStackCorrupted`] if the catch tag root
    /// cannot be released from the thread's root stack.
    pub fn leave_catch(&mut self) -> Result<(), ObjectError> {
        let pending_tag = self
            .thread
            .pending()
            .then(|| self.thread.multiple_values().first().copied())
            .flatten();
        let mut handled = pending_tag.is_none();
        let mut result = if pending_tag.is_some() {
            self.discard_pending_unwind()
        } else {
            Ok(())
        };
        if let Some(frame) = self.frames.pop() {
            self.thread.pop_control_depth(frame.kind());
            if let DynamicFrame::Catch { tag } = frame {
                handled = pending_tag.is_none() || pending_tag == Some(tag.get());
                if let Err(error) = tag.release(self) {
                    result = Err(error);
                }
            }
        }
        if handled {
            self.thread.set_pending(false);
        }
        result
    }

    /// Establish an `unwind-protect` frame.
    pub fn enter_unwind_protect(&mut self, region: u64) {
        let frame = DynamicFrame::UnwindProtect { region };
        self.thread.push_control_depth(frame.kind());
        self.frames.push(frame);
    }

    /// Leave the innermost `unwind-protect` frame.
    ///
    /// This does not touch `pending`: the cleanup forms lowered immediately
    /// after this call run while a non-local exit may still be recorded as
    /// propagating (see the module-level README note on the known gap this
    /// implies for cleanup forms that themselves call further code covered
    /// by an enclosing `catch`).
    ///
    /// # Errors
    ///
    /// Returns [`ObjectError::RootStackCorrupted`] when the saved payload roots
    /// cannot be released in stack order.
    pub fn leave_unwind_protect(&mut self, region: u64) -> Result<(), ObjectError> {
        if matches!(
            self.frames.last(),
            Some(DynamicFrame::UnwindProtect { region: frame_region })
                if *frame_region == region
        ) {
            let Some(frame) = self.frames.pop() else {
                return Ok(());
            };
            self.thread.pop_control_depth(frame.kind());
            if self.thread.pending() {
                let count = self.thread.mv_count();
                let mut values = Vec::with_capacity(count);
                for index in 0..count {
                    let Some(value) = self.thread.multiple_values().get(index).copied() else {
                        return Err(ObjectError::Layout);
                    };
                    values.push(RootedWord::new(self, value));
                }
                self.pending_unwind.push(PendingExit { region, values });
                self.thread.set_pending(false);
            }
            return Ok(());
        }
        if self.pending_unwind.last().map(|saved| saved.region) != Some(region) {
            return Ok(());
        }
        let Some(saved) = self.pending_unwind.pop() else {
            return Ok(());
        };
        let PendingExit {
            region: saved_region,
            values,
        } = saved;
        debug_assert_eq!(saved_region, region);
        if !self.thread.pending() {
            let restored = values.iter().map(RootedWord::get).collect::<Vec<_>>();
            self.thread.set_multiple_value_area(&restored);
            self.thread.set_pending(true);
        }
        let mut result = Ok(());
        for value in values.into_iter().rev() {
            if let Err(error) = value.release(self) {
                result = Err(error);
            }
        }
        result
    }

    /// Establish a `progv` frame, dynamically rebinding each symbol in the
    /// list `symbols` to the corresponding value in `values` (or to `NIL`
    /// when `values` is shorter).
    ///
    /// # Errors
    /// Returns an object error when `symbols`/`values` are not proper lists
    /// of symbols, or when a symbol's value cell cannot be read or written.
    pub fn enter_progv(&mut self, symbols: Word, values: Word) -> Result<(), ObjectError> {
        let mut bindings = Vec::new();
        let mut symbol_cursor = symbols;
        let mut value_cursor = values;
        let result = (|| {
            while symbol_cursor != Word::NIL {
                let symbol = car(self, symbol_cursor)?;
                let value = if value_cursor == Word::NIL {
                    Word::NIL
                } else {
                    car(self, value_cursor)?
                };
                let previous = symbol_value(self, symbol)?;
                bindings.push(ProgvBinding {
                    symbol: RootedWord::new(self, symbol),
                    previous: RootedWord::new(self, previous),
                });
                set_symbol_value(self, symbol, value)?;
                symbol_cursor = cdr(self, symbol_cursor)?;
                if value_cursor != Word::NIL {
                    value_cursor = cdr(self, value_cursor)?;
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            return match restore_progv_bindings(self, bindings) {
                Ok(()) => Err(error),
                Err(rollback_error) => Err(rollback_error),
            };
        }
        let frame = DynamicFrame::Progv { bindings };
        self.thread.push_control_depth(frame.kind());
        self.frames.push(frame);
        Ok(())
    }

    /// Leave the innermost `progv` frame, restoring every rebound symbol's
    /// previous value.
    ///
    /// # Errors
    /// Returns an object error if a bound symbol's value cell can no longer
    /// be written.
    pub fn leave_progv(&mut self) -> Result<(), ObjectError> {
        let mut result = if self.thread.pending() {
            self.discard_pending_unwind()
        } else {
            Ok(())
        };
        let Some(frame) = self.frames.pop() else {
            return result;
        };
        self.thread.pop_control_depth(frame.kind());
        if let DynamicFrame::Progv { bindings } = frame
            && let Err(error) = restore_progv_bindings(self, bindings)
        {
            result = Err(error);
        }
        result
    }

    /// Search the catch chain for `tag` and, if it is currently established,
    /// record the propagating non-local exit that generated code and
    /// [`Self::leave_unwind_protect`]/[`Self::leave_progv`] observe via
    /// `pending`.
    ///
    /// # Errors
    /// Returns [`ObjectError::ControlError`] when no enclosing `catch`
    /// currently established `tag` (this is also how `return-from`/`go`
    /// report an escaping block or tag with no live activation, since both
    /// desugar to `throw` with a compiler-generated tag).
    pub fn throw(&mut self, tag: Word, value: Word) -> Result<(), ObjectError> {
        // Keep the tag/value available to generated dispatch even when this
        // throw escapes all active catches and returns a control error.
        self.thread.set_multiple_value_area(&[tag, value]);
        let established = self.frames.iter().any(
            |frame| matches!(frame, DynamicFrame::Catch { tag: active } if active.get() == tag),
        );
        if !established {
            return Err(ObjectError::ControlError);
        }
        self.thread.set_pending(true);
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use crate::{ObjectError, Runtime, ThreadContext, Word, make_string, make_symbol};

    fn context() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().expect("runtime"); // check-added-lines: allow(panic)
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("register"); // check-added-lines: allow(panic)
        (runtime, ctx)
    }

    #[test]
    fn throw_to_an_established_tag_sets_pending_and_the_payload() {
        let (_runtime, mut ctx) = context();
        let tag = Word::fixnum(7);
        ctx.enter_catch(tag);
        ctx.throw(tag, Word::fixnum(42)).expect("throw"); // check-added-lines: allow(panic)
        assert!(ctx.thread.pending()); // check-added-lines: allow(panic)
        assert_eq!(ctx.thread.mv_count(), 2); // check-added-lines: allow(panic)
        assert_eq!(ctx.thread.multiple_values()[0], tag); // check-added-lines: allow(panic,index)
        assert_eq!(ctx.thread.multiple_values()[1], Word::fixnum(42)); // check-added-lines: allow(panic,index)
        let _ = ctx.leave_catch();
        assert!(!ctx.thread.pending()); // check-added-lines: allow(panic)
    }

    #[test]
    fn throw_to_a_missing_tag_is_a_control_error() {
        let (_runtime, mut ctx) = context();
        let tag = Word::fixnum(9);
        let value = Word::fixnum(1);
        let result = ctx.throw(tag, value);
        assert_eq!(result, Err(ObjectError::ControlError));
        assert!(!ctx.thread.pending());
        assert_eq!(ctx.thread.multiple_values()[0], tag);
        assert_eq!(ctx.thread.multiple_values()[1], value);
    }

    #[test]
    fn throw_finds_an_outer_tag_while_an_inner_catch_is_also_active() {
        let (_runtime, mut ctx) = context();
        let outer = Word::fixnum(1);
        let inner = Word::fixnum(2);
        ctx.enter_catch(outer);
        ctx.enter_catch(inner);
        // Selecting the innermost *matching* frame is generated code's job
        // (it dispatches to whichever of its own open regions' tags match,
        // innermost first); this only exercises that the frame search does
        // not stop at a non-matching innermost frame.
        ctx.throw(outer, Word::fixnum(99)).expect("throw"); // check-added-lines: allow(panic)
        assert!(ctx.thread.pending());
        assert_eq!(ctx.thread.multiple_values()[0], outer);
        let _ = ctx.leave_catch();
        assert!(ctx.thread.pending());
        let _ = ctx.leave_catch();
        assert!(!ctx.thread.pending());
    }

    #[test]
    fn unwind_protect_pass_through_leaves_pending_set_for_the_enclosing_catch() {
        let (_runtime, mut ctx) = context();
        let tag = Word::fixnum(3);
        ctx.enter_catch(tag);
        ctx.enter_unwind_protect(1);
        ctx.throw(tag, Word::fixnum(5)).expect("throw");
        // Leaving unwind-protect on the pass-through path pops its frame but
        // does not touch `pending`, since the cleanup forms run next and the
        // exit is still propagating outward to the enclosing catch.
        ctx.leave_unwind_protect(1).expect("save pending exit");
        ctx.leave_unwind_protect(1).expect("restore pending exit");
        assert!(ctx.thread.pending());
        assert_eq!(ctx.thread.multiple_values()[0], tag);
        assert_eq!(ctx.thread.multiple_values()[1], Word::fixnum(5));
        let _ = ctx.leave_catch();
        assert!(!ctx.thread.pending());
    }

    #[test]
    fn progv_binds_and_restores_the_previous_value() {
        let (runtime, mut ctx) = context();
        let name = make_string(&mut ctx, &runtime, &['X']).expect("name");
        let symbol = make_symbol(&mut ctx, &runtime, name).expect("symbol");
        crate::set_symbol_value(&mut ctx, symbol, Word::fixnum(1)).expect("seed");
        let list = crate::make_cons(&mut ctx, &runtime, symbol, Word::NIL).expect("list");
        let values = crate::make_cons(&mut ctx, &runtime, Word::fixnum(2), Word::NIL).expect("vs");
        ctx.enter_progv(list, values).expect("enter");
        assert_eq!(crate::symbol_value(&ctx, symbol), Ok(Word::fixnum(2)));
        ctx.leave_progv().expect("leave");
        assert_eq!(crate::symbol_value(&ctx, symbol), Ok(Word::fixnum(1)));
    }

    #[test]
    fn thrown_heap_value_survives_gc_across_several_frames_under_stress_and_forwarding() {
        let (runtime, mut ctx) = context();
        ctx.set_gc_stress(true);
        ctx.set_strict_forwarding(true);
        let tag = Word::fixnum(11);
        ctx.enter_catch(tag);
        // Simulate several intervening `unwind-protect` frames (as if the
        // throw were several native call frames deep) that must not disturb
        // the payload while GC runs during their own bookkeeping.
        ctx.enter_unwind_protect(1);
        ctx.enter_unwind_protect(2);
        ctx.enter_unwind_protect(3);

        // A freshly-consed value: nothing but the throw's own bookkeeping
        // roots it once thrown.
        let thrown = crate::make_cons(&mut ctx, &runtime, Word::fixnum(7), Word::NIL)
            .expect("cons the thrown value");
        ctx.throw(tag, thrown).expect("throw");

        // GC runs (as it would at a safepoint inside any of the intervening
        // frames' own calls) while the exit is in flight.
        ctx.collect(true).expect("collect");

        // The frame chain's own roots (the `catch` tag) must also have
        // survived and still compare correctly post-collection.
        assert!(ctx.thread.pending());
        assert_eq!(ctx.thread.multiple_values()[0], tag);
        let forwarded = ctx.thread.multiple_values()[1];
        assert_eq!(crate::car(&ctx, forwarded), Ok(Word::fixnum(7)));

        ctx.leave_unwind_protect(3).expect("save pending exit");
        ctx.leave_unwind_protect(3).expect("restore pending exit");
        ctx.leave_unwind_protect(2).expect("leave middle frame");
        ctx.leave_unwind_protect(1).expect("leave outer frame");
        let _ = ctx.leave_catch();
        assert!(!ctx.thread.pending());
    }
}
