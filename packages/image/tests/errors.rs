#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert on image failures"
)]

//! Error and side-effect coverage for the public image API.

use ncl_image::{CodeImage, ImageError, load, save};
use ncl_object::{
    ArrayElementType, ArrayOptions, Runtime, ThreadContext, Word, make_array, make_readtable,
    make_stream,
};
use ncl_sys::{CodeError, alloc_code};

fn empty_image() -> Vec<u8> {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    save(&runtime, &mut ctx, &[], &[]).unwrap()
}

fn load_error(bytes: &[u8]) -> ImageError {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    runtime.add_feature("before-failed-load");
    let features_before = runtime.features();
    let error = load(bytes, &runtime, &mut ctx).unwrap_err();
    assert_eq!(runtime.features(), features_before);
    error
}

fn image_with_payload(object_count: u32, root_count: u32, payload: &[u8]) -> Vec<u8> {
    let mut image = empty_image();
    image[16..20].copy_from_slice(&object_count.to_le_bytes());
    image[20..24].copy_from_slice(&root_count.to_le_bytes());
    image[44..48].copy_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
    image.truncate(64);
    image.extend_from_slice(payload);
    image
}

fn image_with_code_payload(payload: &[u8]) -> Vec<u8> {
    let mut image = empty_image();
    image[24..28].copy_from_slice(&1_u32.to_le_bytes());
    image[44..48].copy_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
    image.truncate(64);
    image.extend_from_slice(payload);
    image
}

#[test]
fn code_image_rejects_unpublished_and_out_of_bounds_code() {
    let code = alloc_code(1).unwrap();
    assert_eq!(
        CodeImage::capture(&code, 0, 0, "unpublished"),
        Err(ImageError::Code(CodeError::NotPublished))
    );
    assert_eq!(
        CodeImage::from_raw(vec![1, 2], 3, 0, "bad-entry".to_owned()),
        Err(ImageError::InvalidLayout {
            field: "code entry offset"
        })
    );
}

#[test]
fn malformed_headers_report_precise_errors_without_mutating_runtime() {
    let image = empty_image();

    let mut truncated = image.clone();
    truncated.truncate(7);
    assert_eq!(
        load_error(&truncated),
        ImageError::Truncated {
            offset: 0,
            needed: 8
        }
    );

    let mut bad_magic = image.clone();
    bad_magic[0] = b'X';
    assert_eq!(load_error(&bad_magic), ImageError::BadMagic);

    let mut bad_version = image.clone();
    bad_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        load_error(&bad_version),
        ImageError::UnsupportedVersion {
            found: 2,
            supported: ncl_image::FORMAT_VERSION
        }
    );

    let mut bad_architecture = image.clone();
    bad_architecture[10] = 0;
    assert_eq!(
        load_error(&bad_architecture),
        ImageError::UnknownTag {
            space: "architecture",
            tag: 0
        }
    );

    for (offset, field) in [
        (11, "pointer width"),
        (12, "endianness"),
        (13, "header size"),
    ] {
        let mut bad_field = image.clone();
        bad_field[offset] = bad_field[offset].wrapping_add(1);
        assert_eq!(load_error(&bad_field), ImageError::InvalidLayout { field });
    }

    let mut bad_payload = image;
    bad_payload[44..48].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        load_error(&bad_payload),
        ImageError::Truncated {
            offset: 64,
            needed: u32::MAX as usize
        }
    );
}

#[test]
fn load_rejects_an_image_for_a_different_architecture() {
    let mut image = empty_image();
    image[10] = if cfg!(target_arch = "x86_64") { 2 } else { 1 };

    assert_eq!(
        load_error(&image),
        ImageError::InvalidField {
            field: "architecture"
        }
    );
}

#[test]
fn malformed_code_payload_reports_the_code_layout_error() {
    // Empty function name, entry offset 1, and an empty code body.
    let payload = [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    assert_eq!(
        load_error(&image_with_code_payload(&payload)),
        ImageError::InvalidLayout {
            field: "code entry offset"
        }
    );
}

#[test]
fn saving_an_unsupported_array_reports_kind_and_preserves_features() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    runtime.add_feature("before-failed-save");
    let features_before = runtime.features();
    let array = make_array(
        &mut ctx,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();

    assert_eq!(
        save(&runtime, &mut ctx, &[array], &[]),
        Err(ImageError::UnsupportedKind {
            kind: "non-simple array"
        })
    );
    assert_eq!(runtime.features(), features_before);
}

#[test]
fn saving_readtables_and_streams_reports_their_unsupported_kinds() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    runtime.add_feature("before-unsupported-descriptor");
    let features_before = runtime.features();

    let readtable = make_readtable(&mut ctx, &runtime, Word::NIL, Word::NIL, Word::NIL).unwrap();
    assert_eq!(
        save(&runtime, &mut ctx, &[readtable.into()], &[]),
        Err(ImageError::UnsupportedKind { kind: "readtable" })
    );

    let stream = make_stream(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    assert_eq!(
        save(&runtime, &mut ctx, &[stream.into()], &[]),
        Err(ImageError::UnsupportedKind { kind: "stream" })
    );
    assert_eq!(runtime.features(), features_before);
}

#[test]
fn image_errors_preserve_categories_and_display_text() {
    let object = ncl_object::ObjectError::TypeError;
    let cases = [
        (
            ImageError::Truncated {
                offset: 4,
                needed: 8,
            },
            "image truncated at 4, needed 8 bytes",
        ),
        (ImageError::BadMagic, "not an NCL image"),
        (
            ImageError::UnsupportedVersion {
                found: 9,
                supported: 1,
            },
            "image version 9, this build supports 1",
        ),
        (
            ImageError::InvalidField {
                field: "endianness",
            },
            "invalid image field: endianness",
        ),
        (
            ImageError::UnknownTag {
                space: "reference",
                tag: 7,
            },
            "unknown reference tag 7",
        ),
        (
            ImageError::UnsupportedKind { kind: "stream" },
            "unsupported object kind: stream",
        ),
        (
            ImageError::InvalidLayout {
                field: "object count",
            },
            "image layout overflow: object count",
        ),
        (ImageError::Object(object), "object error: TypeError"),
        (
            ImageError::Code(ncl_sys::CodeError::NotPublished),
            "code error: NotPublished",
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
    }
    assert!(std::error::Error::source(&ImageError::Object(object)).is_some());
    assert!(std::error::Error::source(&ImageError::BadMagic).is_none());
    assert_eq!(ImageError::from(object), ImageError::Object(object));
}

#[test]
fn malformed_records_and_references_report_their_decode_categories() {
    assert_eq!(
        load_error(&image_with_payload(1, 0, &[255])),
        ImageError::UnknownTag {
            space: "object kind",
            tag: 255,
        }
    );

    assert_eq!(
        load_error(&image_with_payload(0, 1, &[2])),
        ImageError::UnknownTag {
            space: "reference",
            tag: 2,
        }
    );

    assert_eq!(
        load_error(&image_with_payload(1, 0, &[6, 255, 0, 0, 0, 0, 0])),
        ImageError::InvalidLayout { field: "hash test" }
    );
    assert_eq!(
        load_error(&image_with_payload(1, 0, &[6, 0, 255, 0, 0, 0, 0])),
        ImageError::InvalidLayout {
            field: "hash weakness"
        }
    );

    let invalid_utf8 = image_with_payload(1, 0, &[3, 1, 0, 0, 0, 0xff]);
    assert_eq!(
        load_error(&invalid_utf8),
        ImageError::InvalidLayout {
            field: "string encoding"
        }
    );

    let bad_root = image_with_payload(0, 1, &[1, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(
        load_error(&bad_root),
        ImageError::InvalidLayout {
            field: "object reference"
        }
    );
}

#[test]
fn truncated_feature_payload_reports_the_feature_string_boundary() {
    let mut image = empty_image();
    image[28..32].copy_from_slice(&1_u32.to_le_bytes());
    image[44..48].copy_from_slice(&1_u32.to_le_bytes());
    image.truncate(64);
    image.push(0);

    assert_eq!(
        load_error(&image),
        ImageError::Truncated {
            offset: 0,
            needed: 4,
        }
    );
}

#[test]
fn malformed_object_payload_references_fail_during_reconstruction() {
    // A one-object image whose cons points at a nonexistent second object.
    let cons = [0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0];
    assert_eq!(
        load_error(&image_with_payload(1, 0, &cons)),
        ImageError::InvalidLayout {
            field: "object reference"
        }
    );

    // The same invalid reference through a vector payload exercises the
    // vector branch of the reconstruction pass.
    let vector = [4, 1, 0, 0, 0, 1, 1, 0, 0, 0];
    assert_eq!(
        load_error(&image_with_payload(1, 0, &vector)),
        ImageError::InvalidLayout {
            field: "object reference"
        }
    );
}
