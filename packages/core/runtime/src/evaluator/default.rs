macro_rules! evaluator_default {
    () => {
impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

    };
}
