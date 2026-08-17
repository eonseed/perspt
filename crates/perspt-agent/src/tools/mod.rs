//! Agent Tooling
//!
//! Tools available to agents for interacting with the workspace.
//! File and search operations plus the open handler registry the
//! governed candidate dispatches through.

use diffy::{apply, Patch};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod executor;
pub mod handlers;
mod sandbox;
#[cfg(test)]
mod sandbox_tests;
#[cfg(test)]
mod tests;

pub use executor::*;
pub use sandbox::*;
