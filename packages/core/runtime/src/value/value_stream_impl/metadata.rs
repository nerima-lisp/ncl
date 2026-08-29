use crate::Stream;
use crate::value::value_stream::StreamKind;

impl Stream {
    pub(crate) const fn kind_name(&self) -> &'static str {
        match &self.kind {
            StreamKind::Input { file, .. } => {
                if *file {
                    "FILE-INPUT-STREAM"
                } else {
                    "STRING-INPUT-STREAM"
                }
            }
            StreamKind::Io { .. } => "FILE-IO-STREAM",
            StreamKind::Output { file_path, .. } => {
                if file_path.is_some() {
                    "FILE-OUTPUT-STREAM"
                } else {
                    "STRING-OUTPUT-STREAM"
                }
            }
        }
    }

    pub(crate) const fn is_input(&self) -> bool {
        matches!(&self.kind, StreamKind::Input { .. } | StreamKind::Io { .. })
    }

    pub(crate) const fn is_output(&self) -> bool {
        matches!(
            &self.kind,
            StreamKind::Output { .. } | StreamKind::Io { .. }
        )
    }
}
