//! SB-EXT garbage-collection symbol ownership.

use crate::{Runtime, Word};

/// Register the 14 SB-EXT GC, weak-pointer, and finalizer symbols owned here.
pub fn register(runtime: &Runtime) {
    for name in [
        "*AFTER-GC-HOOKS*",
        "*GC-REAL-TIME*",
        "*GC-RUN-TIME*",
        "CANCEL-FINALIZATION",
        "FINALIZE",
        "GC",
        "GENERATION-BYTES-ALLOCATED",
        "HASH-TABLE-WEAKNESS",
        "MAKE-WEAK-POINTER",
        "MAKE-WEAK-VECTOR",
        "WEAK-POINTER",
        "WEAK-POINTER-P",
        "WEAK-POINTER-VALUE",
        "WEAK-VECTOR-P",
    ] {
        runtime.define_function("SB-EXT", name, Word::UNBOUND);
    }
}
