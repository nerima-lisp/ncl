use super::{
    AccessWidth, Alignment, ByteLength, ByteOffset, ForeignError, ForeignRegion, SapAddress,
    move_sap, read_sap, write_sap,
};

fn region(length: u64) -> Result<ForeignRegion, ForeignError> {
    let alignment = Alignment::new(8)?;
    ForeignRegion::allocate(ByteLength::new(length), alignment)
}

fn offset(address: SapAddress, bytes: u64) -> Result<SapAddress, ForeignError> {
    address.checked_add(ByteOffset::new(bytes))
}

#[test]
fn allocation_reads_and_writes_every_access_width() -> Result<(), ForeignError> {
    let region = region(16)?;
    let base = region.address();
    assert_eq!(region.len(), ByteLength::new(16));
    assert!(!region.is_empty());

    assert_eq!(region.write(base, AccessWidth::U8, 0xa5), Ok(()));
    assert_eq!(region.read(base, AccessWidth::U8), Ok(0xa5));
    assert_eq!(
        region.write(offset(base, 2)?, AccessWidth::U16, 0xbeef),
        Ok(())
    );
    assert_eq!(region.read(offset(base, 2)?, AccessWidth::U16), Ok(0xbeef));
    assert_eq!(
        region.write(offset(base, 4)?, AccessWidth::U32, 0x1234_5678),
        Ok(())
    );
    assert_eq!(
        region.read(offset(base, 4)?, AccessWidth::U32),
        Ok(0x1234_5678)
    );
    assert_eq!(
        region.write(offset(base, 8)?, AccessWidth::U64, 0x1234_5678_9abc_def0),
        Ok(())
    );
    assert_eq!(
        region.read(offset(base, 8)?, AccessWidth::U64),
        Ok(0x1234_5678_9abc_def0)
    );
    assert_eq!(read_sap(base, AccessWidth::U8), Ok(0xa5));
    assert_eq!(write_sap(base, AccessWidth::U8, 0x5a), Ok(()));
    assert_eq!(region.read(base, AccessWidth::U8), Ok(0x5a));
    Ok(())
}

#[test]
fn tracked_region_access_accepts_derived_sap_addresses() -> Result<(), ForeignError> {
    let region = region(16)?;
    let interior = offset(region.address(), 8)?;
    assert_eq!(
        write_sap(interior, AccessWidth::U64, 0xfeed_face_cafe_beef),
        Ok(())
    );
    assert_eq!(
        read_sap(interior, AccessWidth::U64),
        Ok(0xfeed_face_cafe_beef)
    );
    assert_eq!(
        region.read(interior, AccessWidth::U64),
        Ok(0xfeed_face_cafe_beef)
    );
    Ok(())
}

#[test]
fn access_rejects_exact_end_crossing_misalignment_and_narrow_overflow() -> Result<(), ForeignError>
{
    let region = region(12)?;
    let base = region.address();
    assert_eq!(
        region.read(offset(base, 12)?, AccessWidth::U8),
        Err(ForeignError::OutOfBounds)
    );
    assert_eq!(
        region.read(offset(base, 8)?, AccessWidth::U64),
        Err(ForeignError::OutOfBounds)
    );
    assert_eq!(
        region.read(offset(base, 1)?, AccessWidth::U64),
        Err(ForeignError::Misaligned)
    );
    assert_eq!(
        region.write(base, AccessWidth::U8, 0x100),
        Err(ForeignError::ValueOutOfRange)
    );
    Ok(())
}

#[test]
fn null_untracked_and_expired_addresses_fail_without_dereference() -> Result<(), ForeignError> {
    let null = SapAddress::new(0)?;
    assert_eq!(
        read_sap(null, AccessWidth::U8),
        Err(ForeignError::NullAddress)
    );

    let maximum_pointer = u64::try_from(usize::MAX).map_err(|_| ForeignError::AddressOutOfRange)?;
    let untracked = SapAddress::new(maximum_pointer)?;
    assert_eq!(
        read_sap(untracked, AccessWidth::U8),
        Err(ForeignError::UnknownRegion)
    );

    let expired = {
        let region = region(8)?;
        let address = region.address();
        assert_eq!(read_sap(address, AccessWidth::U8), Ok(0));
        address
    };
    assert_eq!(
        read_sap(expired, AccessWidth::U8),
        Err(ForeignError::UnknownRegion)
    );
    Ok(())
}

#[cfg(target_pointer_width = "64")]
#[test]
fn sap_arithmetic_rejects_overflow() -> Result<(), ForeignError> {
    let maximum = SapAddress::new(u64::MAX)?;
    assert_eq!(
        maximum.checked_add(ByteOffset::new(1)),
        Err(ForeignError::AddressOverflow)
    );
    Ok(())
}

#[test]
fn allocation_rejects_empty_regions_and_invalid_alignment() -> Result<(), ForeignError> {
    assert_eq!(Alignment::new(3), Err(ForeignError::InvalidAlignment));
    assert_eq!(Alignment::new(0), Err(ForeignError::InvalidAlignment));
    let alignment = Alignment::new(1)?;
    assert!(matches!(
        ForeignRegion::allocate(ByteLength::new(0), alignment),
        Err(ForeignError::EmptyRegion)
    ));
    Ok(())
}

#[test]
fn tracked_copy_has_memmove_overlap_semantics_and_checks_ranges() -> Result<(), ForeignError> {
    let region = region(8)?;
    let base = region.address();
    for byte in 0..8 {
        assert_eq!(
            region.write(offset(base, byte)?, AccessWidth::U8, byte),
            Ok(())
        );
    }
    assert_eq!(move_sap(offset(base, 1)?, base, ByteLength::new(7)), Ok(()));
    let shifted_right = (0..8)
        .map(|byte| offset(base, byte).and_then(|address| region.read(address, AccessWidth::U8)))
        .collect::<Result<Vec<_>, _>>();
    assert_eq!(shifted_right, Ok(vec![0, 0, 1, 2, 3, 4, 5, 6]));

    assert_eq!(move_sap(base, offset(base, 1)?, ByteLength::new(7)), Ok(()));
    let shifted_left = (0..8)
        .map(|byte| offset(base, byte).and_then(|address| region.read(address, AccessWidth::U8)))
        .collect::<Result<Vec<_>, _>>();
    assert_eq!(shifted_left, Ok(vec![0, 1, 2, 3, 4, 5, 6, 6]));
    assert_eq!(
        move_sap(offset(base, 7)?, base, ByteLength::new(2)),
        Err(ForeignError::OutOfBounds)
    );
    let null = SapAddress::new(0)?;
    assert_eq!(move_sap(null, null, ByteLength::new(0)), Ok(()));
    Ok(())
}
