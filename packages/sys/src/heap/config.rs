use super::HeapConfig;

impl Default for HeapConfig {
    fn default() -> Self {
        Self {
            dynamic_space_size: 64 * 1024 * 1024,
            bytes_considered_between_gcs: 4 * 1024 * 1024,
        }
    }
}
