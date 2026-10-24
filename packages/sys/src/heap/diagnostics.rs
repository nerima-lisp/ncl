use crate::Word;

pub(super) struct WordRejection {
    pub(super) operation: &'static str,
    pub(super) reason: &'static str,
    pub(super) object: Word,
    pub(super) slot: usize,
    pub(super) value: Option<Word>,
    pub(super) forwarding_target: Option<usize>,
    pub(super) object_found: bool,
    pub(super) gc_epoch: u64,
}

#[track_caller]
pub(super) fn log_word_rejection(rejection: WordRejection) {
    if !crate::gc_word_diagnostics_enabled() {
        return;
    }
    let WordRejection {
        operation,
        reason,
        object,
        slot,
        value,
        forwarding_target,
        object_found,
        gc_epoch,
    } = rejection;
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
