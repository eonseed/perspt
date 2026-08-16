//! Agent Tooling
//!
//! Tools available to agents for interacting with the workspace.
//! Implements: read_file, search_code, apply_patch, run_command

use diffy::{apply, Patch};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as AsyncCommand;

mod definitions;
mod executor;
mod sandbox;
#[cfg(test)]
mod sandbox_tests;
#[cfg(test)]
mod tests;

pub use definitions::*;
pub use executor::*;
pub use sandbox::*;
