pub(crate) fn fixed_index(accessor: &str) -> Option<usize> {
    match accessor {
        "SECOND" => Some(1),
        "THIRD" => Some(2),
        "FOURTH" => Some(3),
        "FIFTH" => Some(4),
        "SIXTH" => Some(5),
        "SEVENTH" => Some(6),
        "EIGHTH" => Some(7),
        "NINTH" => Some(8),
        "TENTH" => Some(9),
        _ => None,
    }
}
