#![allow(missing_docs)]

use ncl_lib_hash_arrays::array::{Dimension, Dimensions, RowMajorIndex};

#[test]
fn dimensions_produce_checked_row_major_offsets() {
    let dimensions = Dimensions::new([Dimension::new(2), Dimension::new(3)]).unwrap();

    assert_eq!(dimensions.rank(), 2);
    assert_eq!(dimensions.total_size().unwrap(), 6);
    assert_eq!(
        dimensions
            .row_major_index(&[Dimension::new(1), Dimension::new(2)])
            .unwrap()
            .value(),
        5
    );
}

#[test]
fn checked_offsets_reject_the_total_size() {
    assert!(RowMajorIndex::new(6, 6).is_err());
}
