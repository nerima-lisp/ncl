use super::SectionReader;
use crate::{FaslError, FaslRejectReason};

#[test]
fn section_reader_counts_exact_fits_and_rejects_over_limit_headers() {
    let mut exact_fit = SectionReader::new(
        &[2, 0, 0, 0, 0xaa, 0xbb],
        320,
        FaslRejectReason::InvalidCodeObject,
    );
    assert_eq!(exact_fit.count(2, 1), Ok(2));
    assert_eq!(exact_fit.absolute_position(), 324);

    let mut over_limit =
        SectionReader::new(&[2, 0, 0, 0], 400, FaslRejectReason::InvalidCodeObject);
    assert_eq!(
        over_limit.count(1, 0),
        Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, 400))
    );

    let mut truncated_header =
        SectionReader::new(&[1, 0], 500, FaslRejectReason::InvalidTopLevelRecord);
    assert_eq!(
        truncated_header.count(1, 0),
        Err(FaslError::at(FaslRejectReason::InvalidTopLevelRecord, 500))
    );
}

#[test]
fn section_reader_rejects_counts_that_cannot_fit_the_remaining_bytes() {
    let mut insufficient_records = SectionReader::new(
        &[2, 0, 0, 0, 0xaa],
        700,
        FaslRejectReason::InvalidDebugRecord,
    );
    assert_eq!(
        insufficient_records.count(2, 1),
        Err(FaslError::at(FaslRejectReason::InvalidDebugRecord, 700))
    );

    let mut unbounded_record_size =
        SectionReader::new(&[2, 0, 0, 0], 800, FaslRejectReason::InvalidCodeObject);
    assert_eq!(unbounded_record_size.count(2, 0), Ok(2));
}

#[test]
fn section_reader_take_and_padding_errors_keep_absolute_offsets() {
    let reason = FaslRejectReason::InvalidTopLevelRecord;
    let mut short = SectionReader::new(&[0xaa, 0xbb], 900, reason);
    assert_eq!(short.take(1), Ok(&[0xaa][..]));
    assert_eq!(short.take(2), Err(FaslError::at(reason, 901)));
    assert_eq!(short.remaining(), 1);
    assert_eq!(short.absolute_position(), 901);

    let mut nonzero_padding = SectionReader::new(&[0xaa, 0, 1, 0], 1000, reason);
    assert_eq!(nonzero_padding.take(1), Ok(&[0xaa][..]));
    assert_eq!(
        nonzero_padding.zero_padding(4),
        Err(FaslError::at(reason, 1002))
    );

    let mut truncated_padding = SectionReader::new(&[0xaa], 1100, reason);
    assert_eq!(truncated_padding.take(1), Ok(&[0xaa][..]));
    assert_eq!(
        truncated_padding.zero_padding(4),
        Err(FaslError::at(reason, 1101))
    );
}

#[test]
fn section_reader_primitive_reads_decode_little_endian_values() {
    let bytes = [
        0xaa, 0x34, 0x12, 1, 2, 3, 4, 5, 6, 7, 8, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ];
    let mut reader = SectionReader::new(&bytes, 1400, FaslRejectReason::InvalidCodeObject);
    assert_eq!(reader.u8(), Ok(0xaa));
    assert_eq!(reader.u16(), Ok(0x1234));
    assert_eq!(reader.u64(), Ok(0x0807_0605_0403_0201));
    assert_eq!(reader.i64(), Ok(-2));
    assert_eq!(reader.finish(), Ok(()));
}

#[test]
fn section_reader_primitive_reads_report_truncated_fields_at_absolute_offsets() {
    let reason = FaslRejectReason::InvalidTopLevelRecord;
    let mut short_u8 = SectionReader::new(&[], 1500, reason);
    assert_eq!(short_u8.u8(), Err(FaslError::at(reason, 1500)));

    let mut short_u16 = SectionReader::new(&[0xaa], 1600, reason);
    assert_eq!(short_u16.u16(), Err(FaslError::at(reason, 1600)));

    let mut short_u64 = SectionReader::new(&[0; 7], 1700, reason);
    assert_eq!(short_u64.u64(), Err(FaslError::at(reason, 1700)));

    let mut truncated_i64 = SectionReader::new(&[0; 7], 1800, reason);
    assert_eq!(truncated_i64.i64(), Err(FaslError::at(reason, 1800)));
}

#[test]
fn section_reader_truncated_primitive_uses_the_advanced_absolute_offset() {
    let reason = FaslRejectReason::InvalidCodeObject;
    let mut reader = SectionReader::new(&[0xaa, 0xbb], 2000, reason);
    assert_eq!(reader.u8(), Ok(0xaa));
    assert_eq!(reader.u16(), Err(FaslError::at(reason, 2001)));
    assert_eq!(reader.remaining(), 1);
}

#[test]
fn section_reader_take_rejects_length_addition_overflow() {
    let reason = FaslRejectReason::InvalidCodeObject;
    let mut reader = SectionReader::new(&[0xaa], 1900, reason);
    assert_eq!(reader.take(1), Ok(&[0xaa][..]));
    assert_eq!(reader.take(usize::MAX), Err(FaslError::at(reason, 1901)));
}

#[test]
fn section_reader_finish_requires_the_section_to_end_at_the_cursor() {
    let mut reader = SectionReader::new(&[0xaa, 0xbb], 1200, FaslRejectReason::InvalidCodeObject);
    assert_eq!(reader.take(1), Ok(&[0xaa][..]));
    assert_eq!(
        reader.finish(),
        Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 1201))
    );

    let mut complete = SectionReader::new(&[0xaa], 1300, FaslRejectReason::InvalidCodeObject);
    assert_eq!(complete.take(1), Ok(&[0xaa][..]));
    assert_eq!(complete.finish(), Ok(()));
}
