use crate::Word;

#[track_caller]
pub(super) fn log_word_rejection(
    operation: &'static str,
    reason: &'static str,
    object: Word,
    slot: usize,
    value: Option<Word>,
    forwarding_target: Option<usize>,
    object_found: bool,
    gc_epoch: u64,
) {
    if !crate::gc_word_diagnostics_enabled() {
        return;
    }
    let forwarding_state = match (object_found, forwarding_target) {
        (_, Some(target)) => format!("forwarded target_address={target:#x}"),
        (true, None) => "not_forwarded".to_owned(),
        (false, None) => "unknown".to_owned(),
    };
    eprintln!(
        "NCL_GC_WORD_DIAG operation={operation} reason={reason} object={object:?} address={:#x} slot={slot} value={value:?} forwarding_state={forwarding_state} gc_epoch={gc_epoch} caller={}\n{}",
        object.address(),
        std::panic::Location::caller(),
        std::backtrace::Backtrace::force_capture()
    );
}
