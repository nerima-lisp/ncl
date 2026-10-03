#![allow(missing_docs, clippy::expect_used)]

use ncl_image::{load, save};
use ncl_object::{Runtime, ThreadContext};

#[test]
fn empty_root_image_round_trips_feature_payload_without_roots_or_code() {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("thread registration");
    runtime.add_feature("IMAGE-EMPTY-ROUND-TRIP");

    let bytes = save(&runtime, &mut ctx, &[], &[]).expect("empty image");
    assert_eq!(
        u32::from_le_bytes(bytes[16..20].try_into().expect("image header field")),
        0
    );
    assert_eq!(
        u32::from_le_bytes(bytes[20..24].try_into().expect("image header field")),
        0
    );
    assert_eq!(
        u32::from_le_bytes(bytes[24..28].try_into().expect("image header field")),
        0
    );
    assert_eq!(
        u32::from_le_bytes(bytes[28..32].try_into().expect("image header field")),
        1
    );
    assert_eq!(
        u32::from_le_bytes(bytes[44..48].try_into().expect("image header field")),
        26
    );
    assert_eq!(
        u32::from_le_bytes(bytes[64..68].try_into().expect("image header field")),
        22
    );
    assert_eq!(&bytes[68..], b"IMAGE-EMPTY-ROUND-TRIP");
    let runtime2 = Runtime::new().expect("destination runtime");
    let mut ctx2 = ThreadContext::new();
    ctx2.register(&runtime2).expect("destination registration");
    let loaded = load(&bytes, &runtime2, &mut ctx2).expect("load empty image");

    assert!(loaded.roots.is_empty());
    assert!(loaded.code.is_empty());
}
