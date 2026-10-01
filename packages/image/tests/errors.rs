#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert on image failures"
)]

//! Error and side-effect coverage for the public image API.

use ncl_image::{CodeImage, ImageError, load, save};
use ncl_object::{ArrayElementType, ArrayOptions, Runtime, ThreadContext, Word, make_array};
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
