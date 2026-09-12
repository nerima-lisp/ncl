use crate::Stream;
use crate::value::value_stream::StreamKind;

impl Stream {
    pub(crate) const fn is_open(&self) -> bool {
        !self.closed
    }

    pub(crate) const fn kind_name(&self) -> &'static str {
        match &self.kind {
            StreamKind::Input { file, .. } => {
                if *file {
                    "FILE-INPUT-STREAM"
                } else {
                    "STRING-INPUT-STREAM"
                }
            }
            StreamKind::BinaryInput { .. } => "FILE-INPUT-STREAM",
            StreamKind::Probe => "FILE-STREAM",
            StreamKind::Io { .. } => "FILE-IO-STREAM",
            StreamKind::Output { file_path, .. } => {
                if file_path.is_some() {
                    "FILE-OUTPUT-STREAM"
                } else {
                    "STRING-OUTPUT-STREAM"
                }
            }
            StreamKind::BinaryOutput { .. } => "FILE-OUTPUT-STREAM",
            StreamKind::BinaryIo { .. } => "FILE-IO-STREAM",
        }
    }

    pub(crate) const fn is_input(&self) -> bool {
        matches!(
            &self.kind,
            StreamKind::Input { .. }
                | StreamKind::BinaryInput { .. }
                | StreamKind::Io { .. }
                | StreamKind::BinaryIo { .. }
        )
    }

    pub(crate) const fn is_output(&self) -> bool {
        matches!(
            &self.kind,
            StreamKind::Output { .. }
                | StreamKind::BinaryOutput { .. }
                | StreamKind::Io { .. }
                | StreamKind::BinaryIo { .. }
        )
    }

    pub(crate) const fn is_character_input(&self) -> bool {
        matches!(&self.kind, StreamKind::Input { .. } | StreamKind::Io { .. })
    }

    pub(crate) const fn is_character_output(&self) -> bool {
        matches!(
            &self.kind,
            StreamKind::Output { .. } | StreamKind::Io { .. }
        )
    }

    pub(crate) const fn is_binary_input(&self) -> bool {
        matches!(
            &self.kind,
            StreamKind::BinaryInput { .. } | StreamKind::BinaryIo { .. }
        )
    }

    pub(crate) const fn is_binary_output(&self) -> bool {
        matches!(
            &self.kind,
            StreamKind::BinaryOutput { .. } | StreamKind::BinaryIo { .. }
        )
    }

    pub(crate) const fn element_type_name(&self) -> &'static str {
        if self.is_binary_input() || self.is_binary_output() {
            "UNSIGNED-BYTE"
        } else {
            "CHARACTER"
        }
    }

    pub(crate) const fn is_file_stream(&self) -> bool {
        matches!(
            &self.kind,
            StreamKind::Input { file: true, .. }
                | StreamKind::BinaryInput { .. }
                | StreamKind::Probe
                | StreamKind::Io { .. }
                | StreamKind::Output {
                    file_path: Some(_),
                    ..
                }
                | StreamKind::BinaryOutput { .. }
                | StreamKind::BinaryIo { .. }
        )
    }
}
