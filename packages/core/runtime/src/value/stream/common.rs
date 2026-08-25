use std::{cell::RefCell, rc::Rc};

use super::{Stream, StreamKind};

impl Stream {
    pub(crate) fn is_open(&self) -> bool {
        !self.closed
    }

    pub(crate) fn kind_name(&self) -> &'static str {
        match &self.kind {
            StreamKind::Input { file, .. } => {
                if *file {
                    "FILE-INPUT-STREAM"
                } else {
                    "STRING-INPUT-STREAM"
                }
            }
            StreamKind::Probe => "FILE-PROBE-STREAM",
            StreamKind::Io { .. } => "FILE-IO-STREAM",
            StreamKind::Output { file_path, .. } => {
                if file_path.is_some() {
                    "FILE-OUTPUT-STREAM"
                } else {
                    "STRING-OUTPUT-STREAM"
                }
            }
            StreamKind::TwoWay { .. } => "TWO-WAY-STREAM",
            StreamKind::Broadcast { .. } => "BROADCAST-STREAM",
            StreamKind::Concatenated { .. } => "CONCATENATED-STREAM",
            StreamKind::Echo { .. } => "ECHO-STREAM",
        }
    }

    pub(crate) fn is_input(&self) -> bool {
        matches!(
            &self.kind,
            StreamKind::Input { .. }
                | StreamKind::Io { .. }
                | StreamKind::TwoWay { .. }
                | StreamKind::Concatenated { .. }
                | StreamKind::Echo { .. }
        )
    }

    pub(crate) fn is_output(&self) -> bool {
        matches!(
            &self.kind,
            StreamKind::Output { .. }
                | StreamKind::Io { .. }
                | StreamKind::TwoWay { .. }
                | StreamKind::Broadcast { .. }
                | StreamKind::Echo { .. }
        )
    }

    pub(crate) fn element_type(&self) -> &'static str {
        match &self.kind {
            StreamKind::Broadcast { streams } if streams.is_empty() => "t",
            _ => "character",
        }
    }

    pub(crate) fn broadcast_streams(&self) -> Option<Vec<Rc<RefCell<Stream>>>> {
        match &self.kind {
            StreamKind::Broadcast { streams } => Some(streams.clone()),
            _ => None,
        }
    }

    pub(crate) fn concatenated_streams(&self) -> Option<Vec<Rc<RefCell<Stream>>>> {
        match &self.kind {
            StreamKind::Concatenated { streams, .. } => Some(streams.clone()),
            _ => None,
        }
    }

    pub(crate) fn two_way_input_stream(&self) -> Option<Rc<RefCell<Stream>>> {
        match &self.kind {
            StreamKind::TwoWay { input, .. } => Some(input.clone()),
            _ => None,
        }
    }

    pub(crate) fn two_way_output_stream(&self) -> Option<Rc<RefCell<Stream>>> {
        match &self.kind {
            StreamKind::TwoWay { output, .. } => Some(output.clone()),
            _ => None,
        }
    }

    pub(crate) fn echo_input_stream(&self) -> Option<Rc<RefCell<Stream>>> {
        match &self.kind {
            StreamKind::Echo { input, .. } => Some(input.clone()),
            _ => None,
        }
    }

    pub(crate) fn echo_output_stream(&self) -> Option<Rc<RefCell<Stream>>> {
        match &self.kind {
            StreamKind::Echo { output, .. } => Some(output.clone()),
            _ => None,
        }
    }
}
