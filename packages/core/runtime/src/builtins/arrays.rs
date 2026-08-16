macro_rules! array_builtins {
    () => {
        include!("arrays/lists.rs");
        include!("arrays/creation.rs");
        include!("arrays/access.rs");
        include!("arrays/hash_tables.rs");
        include!("arrays/support.rs");
    };
}
