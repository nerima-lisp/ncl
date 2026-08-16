macro_rules! stream_builtins {
    () => {
        include!("streams/printing.rs");
        include!("streams/reading.rs");
        include!("streams/files.rs");
        include!("streams/characters.rs");
        include!("streams/lifecycle.rs");
    };
}
