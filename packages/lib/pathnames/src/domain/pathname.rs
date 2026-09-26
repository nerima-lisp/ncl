//! The immutable pathname value and component renderers.

use super::{Component, Device, Directory, Host, Name, Type, Version};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Pathname {
    pub host: Option<Host>,
    pub device: Option<Device>,
    pub directory: Option<Directory>,
    pub name: Option<Component>,
    pub type_: Option<Component>,
    pub version: Option<Version>,
}

impl Pathname {
    pub fn new(directory: Option<Directory>, name: Option<Name>, type_: Option<Type>) -> Self {
        Self {
            directory,
            name: name.map(|value| Component::Literal(value.0)),
            type_: type_.map(|value| Component::Literal(value.0)),
            ..Self::default()
        }
    }

    pub fn render_host(&self) -> Option<&str> {
        self.host.as_ref().map(|value| value.0.as_str())
    }
    pub fn render_device(&self) -> Option<&str> {
        self.device.as_ref().map(|value| value.0.as_str())
    }
    pub fn render_directory(&self) -> String {
        let Some(directory) = &self.directory else {
            return String::new();
        };
        let prefix = if directory.kind == super::DirectoryKind::Absolute {
            "/"
        } else {
            ""
        };
        let body = directory
            .elements
            .iter()
            .map(|element| element.render())
            .collect::<Vec<_>>()
            .join("/");
        if body.is_empty() {
            prefix.to_string()
        } else {
            format!("{prefix}{body}/")
        }
    }
    pub fn render_name(&self) -> Option<&str> {
        self.name.as_ref().map(Component::render)
    }
    pub fn render_type(&self) -> Option<&str> {
        self.type_.as_ref().map(Component::render)
    }
    pub fn render_version(&self) -> Option<&str> {
        self.version.as_ref().map(|value| value.0.as_str())
    }

    pub fn namestring(&self) -> String {
        let mut result = String::new();
        if let Some(host) = self.render_host() {
            result.push_str(host);
            result.push(':');
        }
        if let Some(device) = self.render_device() {
            result.push_str(device);
            result.push(':');
        }
        result.push_str(&self.render_directory());
        if let Some(name) = self.render_name() {
            result.push_str(name);
            if let Some(type_) = self.render_type() {
                result.push('.');
                result.push_str(type_);
            }
            if let Some(version) = self.render_version() {
                result.push(';');
                result.push_str(version);
            }
        }
        result
    }
}
