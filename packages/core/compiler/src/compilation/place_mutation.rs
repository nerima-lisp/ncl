#![cfg_attr(test, allow(clippy::wildcard_imports))]
#[cfg(test)]
use super::*;

mod modify;
mod setf;
mod setf_fallback;
#[cfg(test)]
mod tests;
