use super::Thread;
use crate::Word;

impl Thread {
    /// Capture the current generated frame header for collection.
    ///
    /// # Safety
    ///
    /// `frame_fp` must point to a live generated frame whose `frame_words`
    /// words are readable, and `return_pc` must be that frame's continuation PC.
    pub unsafe fn set_native_frame(&mut self, frame_fp: usize, return_pc: usize) {
        // SAFETY: this compatibility entry point has no register snapshot from its caller.
        unsafe { self.set_native_frame_with_registers(frame_fp, return_pc, [0; 16]) };
    }

    /// Capture a generated frame together with the callee-saved register snapshot taken at callback entry.
    ///
    /// # Safety
    ///
    /// `frame_fp` must point to a live generated frame whose `frame_words` words are readable, and
    /// `return_pc` must be its continuation PC. `registers` must be captured before the callback
    /// performs any operation that can reuse the callee-saved registers.
    pub unsafe fn set_native_frame_with_registers(
        &mut self,
        frame_fp: usize,
        return_pc: usize,
        registers: [u64; 16],
    ) {
        let map = self
            .heap()
            .and_then(|heap| heap.safepoint_map_for_pc(return_pc));
        let Some(map) = map else {
            self.frame_snapshot_failed = true;
            self.frame_chain.clear();
            self.frame_registers.clear();
            self.frame_address = None;
            return;
        };
        let frame_words = usize::from(map.frame_words);
        let words = frame_fp as *const Word;
        let mut snapshot = Vec::with_capacity(frame_words);
        // SAFETY: the caller guarantees the generated frame has the mapped width.
        unsafe {
            for index in 0..4 {
                snapshot.push(words.add(index).read());
            }
            for index in 4..frame_words {
                snapshot.push(words.sub(index - 3).read());
            }
        }
        self.frame_chain = snapshot;
        self.stack_bounds = None;
        self.frame_registers = registers.into_iter().map(Word::from_bits).collect();
        self.callee_saved = registers;
        self.frame_chain[1] = Word::from_bits(return_pc as u64);
        self.frame_address = Some(frame_fp);
        self.frame_snapshot_failed = false;
    }

    /// Capture a callback-provided generated frame.
    pub fn capture_native_frame(&mut self, frame_fp: usize, return_pc: usize) {
        // SAFETY: this entry point is called by the generated safepoint callback with its live frame.
        unsafe { self.set_native_frame(frame_fp, return_pc) };
    }

    /// Capture a generated frame using a register snapshot taken at callback entry.
    pub(crate) fn capture_native_frame_with_registers(
        &mut self,
        frame_fp: usize,
        return_pc: usize,
        registers: [u64; 16],
    ) {
        // SAFETY: the generated safepoint callback supplies a live frame and its entry-time register snapshot.
        unsafe { self.set_native_frame_with_registers(frame_fp, return_pc, registers) };
    }
}
