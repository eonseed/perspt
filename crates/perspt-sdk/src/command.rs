//! Typed command IR and governance tiers (PSP-8 System 8).
//!
//! `sh -c` is not an implicit compatibility path. A command proposal is parsed
//! into a typed [`CommandInvocation`]; verifier commands prefer the `Program`
//! form, and the `Shell` form requires a capability that explicitly names shell
//! execution. Coreutils, `awk`, and `sed` are modeled in three tiers so that a
//! read-only inspection cannot silently become a workspace mutation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A canonicalized command invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "form", rename_all = "snake_case")]
pub enum CommandInvocation {
    /// A direct program execution with explicit args — the preferred form.
    Program {
        program: String,
        args: Vec<String>,
        cwd: String,
        env: BTreeMap<String, String>,
    },
    /// A shell script — requires an explicit `RunShell` capability.
    Shell {
        script: String,
        cwd: String,
        declared_reads: Vec<String>,
        declared_writes: Vec<String>,
    },
}

impl CommandInvocation {
    /// Whether this invocation requires a shell capability.
    pub fn requires_shell(&self) -> bool {
        matches!(self, CommandInvocation::Shell { .. })
    }

    /// The program name being invoked (for `Program`) or `"sh"` for `Shell`.
    pub fn program_name(&self) -> &str {
        match self {
            CommandInvocation::Program { program, .. } => program,
            CommandInvocation::Shell { .. } => "sh",
        }
    }

    /// The canonical single-line rendering of this invocation — the stable
    /// input contract for text-matching policy layers. `Program` renders as
    /// the shell-quoted argv; `Shell` renders its script verbatim. Never feed
    /// a policy the Rust `Debug` form: its field syntax defeats every
    /// substring pattern written against real command lines.
    pub fn command_line(&self) -> String {
        match self {
            CommandInvocation::Program { program, args, .. } => {
                let mut parts = Vec::with_capacity(args.len() + 1);
                parts.push(program.clone());
                parts.extend(args.iter().cloned());
                shell_words::join(parts.iter().map(String::as_str))
            }
            CommandInvocation::Shell { script, .. } => script.clone(),
        }
    }
}

/// Shell metacharacters that force the `Shell` form. Their presence means the
/// command cannot be canonicalized to a single program execution.
const SHELL_METACHARS: &[char] = &[
    '|', '&', ';', '<', '>', '$', '`', '(', ')', '{', '}', '*', '?', '~', '!', '\n',
];

/// Whether a raw command string contains shell composition.
pub fn has_shell_composition(raw: &str) -> bool {
    raw.chars().any(|c| SHELL_METACHARS.contains(&c))
}

/// Canonicalize a raw command string. A command free of shell composition is
/// parsed into the `Program` form; otherwise it is a `Shell` invocation that
/// requires a shell capability. Parsing is intentionally simple and
/// shell-free: quoting is parsed as argv syntax, while composition operators
/// still force the `Shell` form. Invalid argv syntax also fails closed to the
/// shell form and therefore requires explicit shell authority.
pub fn canonicalize(raw: &str, cwd: &str) -> CommandInvocation {
    if has_shell_composition(raw) {
        return CommandInvocation::Shell {
            script: raw.to_string(),
            cwd: cwd.to_string(),
            declared_reads: Vec::new(),
            declared_writes: Vec::new(),
        };
    }
    let mut tokens = match shell_words::split(raw) {
        Ok(tokens) => tokens.into_iter(),
        Err(_) => {
            return CommandInvocation::Shell {
                script: raw.to_string(),
                cwd: cwd.to_string(),
                declared_reads: Vec::new(),
                declared_writes: Vec::new(),
            };
        }
    };
    match tokens.next() {
        Some(program) => CommandInvocation::Program {
            program,
            args: tokens.collect(),
            cwd: cwd.to_string(),
            env: BTreeMap::new(),
        },
        None => CommandInvocation::Shell {
            script: String::new(),
            cwd: cwd.to_string(),
            declared_reads: Vec::new(),
            declared_writes: Vec::new(),
        },
    }
}

/// Governance tier for coreutils / `awk` / `sed` style commands (PSP-8 System 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandTier {
    /// Read-only commands that can produce residual evidence.
    Inspection,
    /// Transformations that produce a proposed diff but do not mutate files.
    PatchPreview,
    /// Commands that directly modify the workspace — require mutation capability.
    Mutation,
}

/// Classify a program invocation into a governance tier. This is a conservative
/// default: anything not recognized as read-only is treated as a mutation so a
/// novel tool cannot slip through as inspection.
pub fn classify_tier(invocation: &CommandInvocation) -> CommandTier {
    let (program, args) = match invocation {
        CommandInvocation::Program { program, args, .. } => (program.as_str(), args.as_slice()),
        // A declaration is an assertion, not proof. Shell composition is
        // always mutation-tier; a process sandbox may still enforce a
        // read-only filesystem for explicitly approved inspection scripts.
        CommandInvocation::Shell { .. } => return CommandTier::Mutation,
    };

    let base = program.rsplit('/').next().unwrap_or(program);
    match base {
        // Always read-only.
        "rg" | "grep" | "find" | "sort" | "uniq" | "wc" | "comm" | "cat" | "head" | "tail"
        | "ls" | "git-grep" => CommandTier::Inspection,
        // `git grep`, `git diff`, `git status` are read-only; `git` others vary.
        "git" => match args.first().map(String::as_str) {
            Some("grep") | Some("diff") | Some("status") | Some("log") | Some("show") => {
                CommandTier::Inspection
            }
            _ => CommandTier::Mutation,
        },
        // `sed -n` and plain `awk` filters are read-only; `sed -i` mutates.
        "sed" => {
            if args.iter().any(|a| a == "-i" || a.starts_with("-i")) {
                CommandTier::Mutation
            } else if args.iter().any(|a| a == "-n") {
                CommandTier::Inspection
            } else {
                CommandTier::PatchPreview
            }
        }
        "awk" => CommandTier::Inspection,
        // Package managers and removers mutate.
        "rm" | "mv" | "cp" | "cargo" | "npm" | "pnpm" | "yarn" | "pip" | "uv" | "go" => {
            CommandTier::Mutation
        }
        _ => CommandTier::Mutation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_command_is_program_form() {
        let cmd = canonicalize("cargo check --workspace", "/repo");
        assert!(matches!(cmd, CommandInvocation::Program { .. }));
        assert!(!cmd.requires_shell());
        assert_eq!(cmd.program_name(), "cargo");
    }

    #[test]
    fn piped_command_is_shell_form() {
        let cmd = canonicalize("cat x | grep y", "/repo");
        assert!(cmd.requires_shell());
    }

    #[test]
    fn quotes_preserve_a_direct_program_invocation() {
        let cmd = canonicalize(r#"rg "hello world" src"#, "/repo");
        assert_eq!(
            cmd,
            CommandInvocation::Program {
                program: "rg".into(),
                args: vec!["hello world".into(), "src".into()],
                cwd: "/repo".into(),
                env: BTreeMap::new(),
            }
        );
    }

    #[test]
    fn an_empty_shell_write_declaration_is_not_read_only_proof() {
        let command = CommandInvocation::Shell {
            script: "cat x | rg y".into(),
            cwd: "/repo".into(),
            declared_reads: vec!["x".into()],
            declared_writes: Vec::new(),
        };
        assert_eq!(classify_tier(&command), CommandTier::Mutation);
    }

    #[test]
    fn redirect_forces_shell_form() {
        assert!(canonicalize("echo hi > f", "/repo").requires_shell());
        assert!(canonicalize("rm -rf $HOME", "/repo").requires_shell());
    }

    #[test]
    fn read_only_tools_are_inspection() {
        assert_eq!(
            classify_tier(&canonicalize("rg pattern", "/r")),
            CommandTier::Inspection
        );
        assert_eq!(
            classify_tier(&canonicalize("git grep foo", "/r")),
            CommandTier::Inspection
        );
        assert_eq!(
            classify_tier(&canonicalize("sed -n 1p file", "/r")),
            CommandTier::Inspection
        );
    }

    #[test]
    fn sed_in_place_is_mutation() {
        assert_eq!(
            classify_tier(&canonicalize("sed -i s/a/b/ file", "/r")),
            CommandTier::Mutation
        );
    }

    #[test]
    fn package_managers_are_mutation() {
        assert_eq!(
            classify_tier(&canonicalize("cargo add serde", "/r")),
            CommandTier::Mutation
        );
        assert_eq!(
            classify_tier(&canonicalize("rm file", "/r")),
            CommandTier::Mutation
        );
    }

    #[test]
    fn unknown_tool_defaults_to_mutation() {
        assert_eq!(
            classify_tier(&canonicalize("frobnicate x", "/r")),
            CommandTier::Mutation
        );
    }
}
