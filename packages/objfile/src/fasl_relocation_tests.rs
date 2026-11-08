use super::*;
use FaslRelocationKind::{Abs64, ConstantIndex32};

fn item(kind: FaslRelocationKind) -> FaslRelocation {
    FaslRelocation {
        code_object_id: 0,
        write_offset: 4,
        kind,
        target_kind: FaslRelocationTarget::CodeObject,
        target_index: 0,
        addend: 0,
        pair_id: 0,
    }
}

fn must_encode(items: &[FaslRelocation]) -> Vec<u8> {
    encode_v1_section(items).unwrap_or_else(|_| panic!("invalid relocation test fixture"))
}

fn bad_fasl(bytes: &[u8]) {
    assert_eq!(
        decode_v1_section(bytes, 0).map_err(|error| error.reason),
        Err(FaslRejectReason::InvalidRelocation)
    );
}

fn legacy(symbol: SymbolRef, addend: i64) -> Relocation {
    Relocation {
        section: SectionId(1),
        offset: 4,
        kind: RelocKind::PcRel32,
        symbol,
        addend,
    }
}

#[test]
fn codecs_check_v1_layout_and_legacy_symbol_records() {
    assert_eq!((Abs64.width(), ConstantIndex32.width()), (8, 4));
    let constant = FaslRelocation {
        target_kind: FaslRelocationTarget::ConstantVector,
        target_index: 9,
        addend: i64::MIN,
        ..item(ConstantIndex32)
    };
    let encoded = must_encode(&[constant]);
    assert_eq!(&encoded[12..16], &9_u32.to_le_bytes());
    assert_eq!(&encoded[16..20], &3_u32.to_le_bytes());
    assert_eq!(&encoded[24..32], &i64::MIN.to_le_bytes());
    assert_eq!(decode_v1_section(&encoded, 0), Ok(vec![constant]));

    let valid = must_encode(&[item(Abs64)]);
    for (field, value) in [(12, 10_u32), (16, 0), (16, 4)] {
        let mut invalid = valid.clone();
        invalid[field..field + 4].copy_from_slice(&value.to_le_bytes());
        bad_fasl(&invalid);
    }
    bad_fasl(&valid[..35]);

    let first = item(FaslRelocationKind::PcRel32);
    let second = item(Abs64);
    assert!(encode_v1_section(&[first, second]).is_err());
    let later = FaslRelocation {
        write_offset: 8,
        ..second
    };
    let mut unordered = must_encode(&[later, first]);
    unordered[4..68].rotate_left(32);
    bad_fasl(&unordered);

    let local = legacy(SymbolRef::Local(0), -4);
    let external = legacy(SymbolRef::External("native".into()), -4);
    let relocations = vec![local, external];
    let encoded =
        encode(&relocations).unwrap_or_else(|error| panic!("valid legacy fixture: {error}"));
    assert_eq!(decode(&encoded, 2, 2), Ok(relocations.clone()));
    assert_eq!(
        decode(&encoded[..16], 1, 1),
        Ok(vec![relocations[0].clone()])
    );
    assert!(decode(&encoded[..15], 1, 1).is_err());

    for (offset, value) in [(8, 99_u32), (16, 2)] {
        let mut invalid = encoded.clone();
        invalid[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(decode(&invalid, 2, 2).is_err());
    }
    let mut bad_utf8 = encoded.clone();
    bad_utf8[48] = 0xff;
    assert_eq!(decode(&bad_utf8, 2, 2), Err(ObjectError::InvalidName));

    let mut trailing = encoded;
    trailing.push(0);
    assert!(decode(&trailing, 2, 2).is_err());
    let too_large = legacy(SymbolRef::Local(0), i64::from(i32::MAX) + 1);
    assert!(encode(&[too_large]).is_err());
}
