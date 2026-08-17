macro_rules! format_builtins {
    () => {
        include!("format/parameters.rs");
        include!("format/entry.rs");
        include!("format/control.rs");
        include!("format/structure.rs");
        include!("format/layout.rs");
        include!("format/numeric.rs");
        #[cfg(test)]
        mod numeric_tests {
            include!("format/numeric_tests.rs");
        }
        include!("format/values.rs");

    };
}
