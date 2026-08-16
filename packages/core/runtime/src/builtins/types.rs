macro_rules! type_builtins {
    () => {
        include!("types/designators.rs");
        include!("types/errors.rs");
        include!("types/matching.rs");
    };
}
