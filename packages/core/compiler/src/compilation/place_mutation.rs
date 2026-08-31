#![cfg_attr(test, allow(clippy::wildcard_imports))]
#[cfg(test)]
use super::*;

mod modify;
mod setf;
mod setf_emit;
mod setf_fallback;
mod setf_property;
#[cfg(test)]
mod tests;
