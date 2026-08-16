//! Perspt repository automation.
//!
//! Perspt is held to NASA coding rules. This tool encodes them so they can be
//! measured rather than remembered:
//!
//! | Rule | Limit |
//! |---|---|
//! | `NASA-1` file length | 1408 lines |
//! | `NASA-2` function length | 70 code lines |
//! | `NASA-3` line width | 108 columns |
//!
//! ```text
//! ./check-rules.sh check                # CI gate; fails on any new violation
//! ./check-rules.sh report               # every offending file, function, line
//! ./check-rules.sh report --format json # for dashboards and scripts
//! ./check-rules.sh report --rule NASA-2 # one rule
//! ./check-rules.sh baseline --shrink    # ratchet accepted debt downward
//! ```
//!
//! `.cargo/config.toml` is gitignored, so `check-rules.sh` rather than a
//! `cargo xtask` alias is the entry point that survives a fresh clone.
//!
//! The rules cover Rust sources only. `docs/` — the PSPs and the Sphinx book —
//! is out of scope.

mod baseline;
mod report;
mod rules;
mod scan;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use baseline::Baseline;
use report::Format;
use rules::{RuleId, Violation};

#[derive(Parser)]
#[command(name = "xtask", about = "Perspt repository automation", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Fail if any file breaks a rule beyond what the baseline accepts.
    Check {
        /// Restrict to one rule, e.g. `NASA-2`.
        #[arg(long, value_name = "CODE")]
        rule: Option<String>,
    },
    /// List every file, function, and line that breaks a rule.
    Report {
        #[arg(long, value_enum, default_value_t = Format::Table)]
        format: Format,
        /// Restrict to one rule, e.g. `NASA-2`.
        #[arg(long, value_name = "CODE")]
        rule: Option<String>,
    },
    /// Record current violations as accepted debt.
    Baseline {
        /// Only lower existing counts; refuse to record anything new.
        #[arg(long)]
        shrink: bool,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("xtask: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    let root = repo_root()?;
    match Cli::parse().command {
        Command::Check { rule } => cmd_check(&root, parse_rule(rule.as_deref())?),
        Command::Report { format, rule } => cmd_report(&root, format, parse_rule(rule.as_deref())?),
        Command::Baseline { shrink } => cmd_baseline(&root, shrink),
    }
}

/// Enforce the ratchet: a count may shrink or hold, never grow.
fn cmd_check(root: &Path, only: Option<RuleId>) -> Result<ExitCode> {
    let violations = analyze(root, only)?;
    let accepted = Baseline::load(root)?;
    let (regressions, improvements) = baseline::evaluate(&violations, &accepted);

    if regressions.is_empty() {
        report_clean(&violations, &improvements, &accepted)?;
        return Ok(ExitCode::SUCCESS);
    }

    emit(&format!(
        "NASA coding rules: {} regression(s).\n\n",
        regressions.len()
    ))?;
    for regression in &regressions {
        emit(&format!(
            "{}  {}  {} violation(s), baseline allows {}\n",
            regression.rule.code(),
            regression.file,
            regression.found,
            regression.allowed,
        ))?;
        let offending: Vec<Violation> = violations
            .iter()
            .filter(|v| {
                v.rule == regression.rule && v.file.display().to_string() == regression.file
            })
            .cloned()
            .collect();
        emit(&indent(&report::render(&offending, Format::Table)?))?;
    }
    emit("Decompose the file or shorten the function. Do not raise the baseline.\n")?;
    Ok(ExitCode::FAILURE)
}

/// Print the details of every violation, baselined or not.
fn cmd_report(root: &Path, format: Format, only: Option<RuleId>) -> Result<ExitCode> {
    let violations = analyze(root, only)?;
    emit(&report::render(&violations, format)?)?;
    Ok(ExitCode::SUCCESS)
}

/// Rewrite the baseline, optionally refusing to accept anything new.
fn cmd_baseline(root: &Path, shrink: bool) -> Result<ExitCode> {
    let violations = analyze(root, None)?;
    let current = Baseline::from_violations(&violations);

    if shrink {
        let accepted = Baseline::load(root)?;
        let (regressions, _) = baseline::evaluate(&violations, &accepted);
        if !regressions.is_empty() {
            bail!(
                "--shrink refuses to record {} new violation(s); run `cargo xtask check` \
                 and fix them first",
                regressions.len()
            );
        }
    }

    current.save(root)?;
    if current.is_clean() {
        emit("Baseline is empty: every Rust source obeys all three rules.\n")?;
    } else {
        emit(&format!(
            "Baseline written: {} entr(ies).\n",
            current.allowances.len()
        ))?;
    }
    Ok(ExitCode::SUCCESS)
}

/// Run every enabled rule over every in-scope Rust source.
fn analyze(root: &Path, only: Option<RuleId>) -> Result<Vec<Violation>> {
    let enabled = |rule: RuleId| only.is_none_or(|selected| selected == rule);
    let mut violations = Vec::new();

    for file in scan::discover(root)? {
        if enabled(RuleId::FileLength) {
            violations.extend(rules::file_len::check(&file));
        }
        if enabled(RuleId::LineWidth) {
            violations.extend(rules::line_len::check(&file));
        }
        if enabled(RuleId::FunctionLength) {
            violations.extend(rules::fn_len::check(&file)?);
        }
    }

    Ok(violations)
}

/// Summarize a passing run, including debt that is now ready to be dropped.
fn report_clean(
    violations: &[Violation],
    improvements: &[baseline::Improvement],
    accepted: &Baseline,
) -> Result<()> {
    if violations.is_empty() && accepted.is_clean() {
        emit("NASA coding rules: all Rust sources pass.\n")?;
    } else {
        emit(&format!(
            "NASA coding rules: pass. {} violation(s), all within the accepted baseline.\n",
            violations.len()
        ))?;
    }
    if improvements.is_empty() {
        return Ok(());
    }
    emit(&format!(
        "\n{} baseline entr(ies) improved - run `cargo xtask baseline --shrink`:\n",
        improvements.len()
    ))?;
    for improvement in improvements {
        emit(&format!(
            "  {}  {}  {} -> {}\n",
            improvement.rule.code(),
            improvement.file,
            improvement.allowed,
            improvement.found,
        ))?;
    }
    Ok(())
}

fn parse_rule(code: Option<&str>) -> Result<Option<RuleId>> {
    let Some(code) = code else {
        return Ok(None);
    };
    RuleId::parse(code)
        .map(Some)
        .with_context(|| format!("unknown rule {code:?}; expected NASA-1, NASA-2, or NASA-3"))
}

/// The workspace root, one level above this crate.
fn repo_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("locating the repository root above xtask/")
}

/// Write to stdout, treating a closed pipe as success.
///
/// Reports are long and routinely piped into `head` or `grep`, which close the
/// pipe early. That is the reader's choice, not an error.
fn emit(text: &str) -> Result<()> {
    use std::io::{ErrorKind, Write};
    match std::io::stdout().write_all(text.as_bytes()) {
        Err(error) if error.kind() == ErrorKind::BrokenPipe => Ok(()),
        other => other.context("writing to stdout"),
    }
}

/// Indent a block by two spaces so it nests under its heading.
fn indent(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("  {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_filters_are_parsed_case_insensitively() {
        assert_eq!(parse_rule(None).unwrap(), None);
        assert_eq!(
            parse_rule(Some("nasa-1")).unwrap(),
            Some(RuleId::FileLength)
        );
        assert!(parse_rule(Some("NASA-4")).is_err());
    }

    #[test]
    fn the_repository_root_holds_the_workspace_manifest() {
        let root = repo_root().expect("resolves");
        assert!(root.join("Cargo.toml").exists(), "{}", root.display());
        assert!(root.join("crates").is_dir(), "{}", root.display());
    }

    #[test]
    fn indent_nests_every_non_empty_line() {
        assert_eq!(indent("a\n\nb"), "  a\n\n  b\n");
    }
}
