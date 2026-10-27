use super::*;

#[test]
fn layout_matches_c_struct_offsets() {
    let thread = Thread::new();
    let layout = thread_layout();
    assert_eq!(layout.tlab_bump, std::mem::offset_of!(Thread, tlab_bump));
    assert_eq!(layout.tlab_limit, std::mem::offset_of!(Thread, tlab_limit));
    assert_eq!(
        layout.safepoint_request,
        std::mem::offset_of!(Thread, safepoint_request)
    );
    assert_eq!(layout.pending, std::mem::offset_of!(Thread, pending));
    assert_eq!(layout.mv, std::mem::offset_of!(Thread, mv));
    assert_eq!(layout.handler, std::mem::offset_of!(Thread, handler));
    assert_eq!(layout.cleanup, std::mem::offset_of!(Thread, cleanup));
    assert_eq!(layout.catch, std::mem::offset_of!(Thread, catch));
    assert_eq!(layout.mv_count, std::mem::offset_of!(Thread, mv_count));
    assert_eq!(
        layout.runtime_error_flags,
        std::mem::offset_of!(Thread, runtime_error_flags)
    );
    assert_eq!(
        layout.runtime_error_flags + 8,
        std::mem::offset_of!(Thread, stack_scan_start)
    );
    assert_eq!(
        std::mem::offset_of!(Thread, gc_state) + std::mem::size_of_val(&thread.gc_state),
        std::mem::size_of::<Thread>()
    );
}

#[test]
fn machine_visible_fields_are_aligned_single_words() {
    let thread = Thread::new();
    let layout = thread_layout();
    let fields = [
        (layout.tlab_bump, std::mem::size_of_val(&thread.tlab_bump)),
        (layout.tlab_limit, std::mem::size_of_val(&thread.tlab_limit)),
        (
            layout.safepoint_request,
            std::mem::size_of_val(&thread.safepoint_request),
        ),
        (layout.pending, std::mem::size_of_val(&thread.pending)),
        (layout.handler, std::mem::size_of_val(&thread.handler)),
        (layout.cleanup, std::mem::size_of_val(&thread.cleanup)),
        (layout.catch, std::mem::size_of_val(&thread.catch)),
        (layout.mv_count, std::mem::size_of_val(&thread.mv_count)),
        (
            layout.runtime_error_flags,
            std::mem::size_of_val(&thread.runtime_error_flags),
        ),
    ];
    for (offset, size) in fields {
        assert_eq!(offset % 8, 0);
        assert_eq!(size, 8);
    }
    assert_eq!(layout.mv % 8, 0);
    assert_eq!(
        std::mem::size_of::<[Word; MULTIPLE_VALUE_AREA_WORDS]>(),
        MULTIPLE_VALUE_AREA_WORDS * 8
    );
}
