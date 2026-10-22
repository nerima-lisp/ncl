#![allow(clippy::unwrap_used, reason = "tests mutate valid image fixtures")]

use super::{FORMAT_VERSION, Reader, get_record};
use crate::ImageError;
use crate::image_builtin_support::{BUILTIN, BUILTIN_ID, image_builtin};
use ncl_object::{BuiltinImplementation, Runtime, ThreadContext};

fn registered_builtin_image() -> Vec<u8> {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let function = runtime
        .register_builtin(
            &mut ctx,
            BUILTIN_ID,
            BuiltinImplementation::direct(BUILTIN, image_builtin).with_entry(0x1234_5678),
        )
        .unwrap();
    crate::save(&runtime, &mut ctx, &[function.as_word()], &[]).unwrap()
}

fn builtin_record_offsets(bytes: &[u8]) -> (usize, usize) {
    let object_count = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
    let payload_offset = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    let payload_size = u32::from_le_bytes(bytes[44..48].try_into().unwrap()) as usize;
    let payload = &bytes[payload_offset..payload_offset + payload_size];
    let mut reader = Reader::new(payload);
    for _ in 0..object_count {
        let record_start = reader.pos;
        if let crate::record::Record::Function {
            builtin: Some(identity),
            ..
        } = get_record(&mut reader).unwrap()
        {
            let closure_offset = payload_offset + record_start + 1;
            let identity_tag_offset = payload_offset + reader.pos
                - (1 + 4 + identity.package.len() + 4 + identity.name.len());
            return (closure_offset, identity_tag_offset);
        }
    }
    panic!("saved image has no builtin identity record");
}

#[test]
fn version_one_image_is_rejected_before_builtin_restore() {
    let mut image = registered_builtin_image();
    image[8..10].copy_from_slice(&1_u16.to_le_bytes());
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    assert_eq!(
        crate::load(&image, &runtime, &mut ctx).unwrap_err(),
        ImageError::UnsupportedVersion {
            found: 1,
            supported: FORMAT_VERSION,
        }
    );
}

#[test]
fn unknown_builtin_identity_tag_is_rejected() {
    let mut image = registered_builtin_image();
    let (_, tag_offset) = builtin_record_offsets(&image);
    assert_eq!(image[tag_offset], 1);
    image[tag_offset] = 2;
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    assert_eq!(
        crate::load(&image, &runtime, &mut ctx).unwrap_err(),
        ImageError::UnknownTag {
            space: "builtin identity",
            tag: 2,
        }
    );
}

#[test]
fn closure_with_builtin_identity_is_rejected_on_load() {
    let mut image = registered_builtin_image();
    let (closure_offset, identity_tag_offset) = builtin_record_offsets(&image);
    assert_eq!(image[closure_offset], 0);
    assert_eq!(image[identity_tag_offset], 1);
    image[closure_offset] = 1;

    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    assert_eq!(
        crate::load(&image, &runtime, &mut ctx).unwrap_err(),
        ImageError::InvalidRecord {
            kind: "function",
            reason: "closure has builtin identity",
        }
    );
}
