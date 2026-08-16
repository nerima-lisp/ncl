macro_rules! numeric_builtins {
    () => {
        include!("numeric/arithmetic.rs");
        include!("numeric/rationals.rs");
        include!("numeric/predicates.rs");
        include!("numeric/rounding.rs");
        include!("numeric/integer.rs");
    };
}
