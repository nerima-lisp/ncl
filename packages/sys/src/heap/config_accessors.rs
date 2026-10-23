use super::Heap;

impl Heap {
    /// Return the configured dynamic-space capacity in bytes.
    pub const fn dynamic_space_size(&self) -> usize {
        self.config.dynamic_space_size
    }
    /// Return the allocation debt threshold that triggers collection.
    pub const fn bytes_considered_between_gcs(&self) -> usize {
        self.config.bytes_considered_between_gcs
    }
    /// Return the configured collector metadata limit, if any.
    pub const fn max_metadata_bytes(&self) -> Option<usize> {
        self.options.max_metadata_bytes
    }
    /// Return whether allocation slow paths may request automatic collection.
    pub const fn auto_gc(&self) -> bool {
        self.options.auto_gc
    }
}
