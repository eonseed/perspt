//! System explorer family: read-only operating-system and toolchain facts.
//!
//! Every probe is `EffectKind::SystemProbe` (read-only) with a scoped
//! footprint keyed by the probe name, so probes serialize against
//! themselves, never against the workspace. `sys_env` returns variable
//! **names only** — values are withheld so secrets never enter the
//! conversation.

use std::sync::Arc;

use anyhow::Result;
use perspt_sdk::{AccessMode, EffectKind, FootprintSpec, Resource, ResourceSelector, ToolEntry};

use super::{family_entry, object_schema};
use crate::candidate::CandidateWorkspace;
use crate::toolloop::EffectOutcome;
use crate::tools::handlers::{CandidateHandlerRegistry, CandidateToolHandler};

fn probe_footprint(key: &str) -> FootprintSpec {
    FootprintSpec::new(vec![ResourceSelector::Literal {
        resource: Resource::Scoped {
            family: "system".into(),
            key: key.into(),
        },
        access: AccessMode::Read,
    }])
}

pub fn entries() -> Vec<ToolEntry> {
    vec![
        family_entry(
            "sys_info",
            "Read-only host facts: OS, architecture, and toolchain versions \
             (rustc, cargo, python, uv, node, git)",
            EffectKind::SystemProbe,
            object_schema(&[]),
            probe_footprint("info"),
        ),
        family_entry(
            "sys_processes",
            "List running processes (pid, cpu%, mem%, command name only — \
             never the arguments of other processes)",
            EffectKind::SystemProbe,
            object_schema(&[]),
            probe_footprint("processes"),
        ),
        family_entry(
            "sys_disk",
            "Filesystem usage for a workspace-relative path",
            EffectKind::SystemProbe,
            object_schema(&[(
                "path",
                "string",
                "Workspace-relative path (default workspace root)",
                false,
            )]),
            probe_footprint("disk"),
        ),
        family_entry(
            "sys_env",
            "Environment variable NAMES only; values are withheld so secrets \
             never enter the conversation",
            EffectKind::SystemProbe,
            object_schema(&[]),
            probe_footprint("env"),
        ),
    ]
}

pub fn register(registry: &mut CandidateHandlerRegistry) -> Result<()> {
    registry.register("sys_info", Arc::new(SysInfo))?;
    registry.register("sys_processes", Arc::new(SysProcesses))?;
    registry.register("sys_disk", Arc::new(SysDisk))?;
    registry.register("sys_env", Arc::new(SysEnv))?;
    Ok(())
}

const OUTPUT_CAP: usize = 16 * 1024;

fn capped(mut output: String) -> String {
    if output.len() > OUTPUT_CAP {
        output.truncate(OUTPUT_CAP);
        output.push_str("\n… (truncated)");
    }
    output
}

fn tool_version(program: &str) -> String {
    std::process::Command::new(program)
        .arg("--version")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_else(|| "not found".into())
}

struct SysInfo;

#[async_trait::async_trait]
impl CandidateToolHandler for SysInfo {
    async fn apply(
        &self,
        _workspace: &CandidateWorkspace,
        _call: &perspt_sdk::ProviderToolCall,
        _entry: &ToolEntry,
    ) -> Result<EffectOutcome> {
        let mut lines = vec![
            format!("os: {}", std::env::consts::OS),
            format!("arch: {}", std::env::consts::ARCH),
        ];
        for program in ["rustc", "cargo", "python3", "uv", "node", "git"] {
            lines.push(format!("{program}: {}", tool_version(program)));
        }
        Ok(EffectOutcome {
            output: capped(lines.join("\n")),
            mutated: false,
        })
    }
}

struct SysProcesses;

#[async_trait::async_trait]
impl CandidateToolHandler for SysProcesses {
    async fn apply(
        &self,
        _workspace: &CandidateWorkspace,
        _call: &perspt_sdk::ProviderToolCall,
        _entry: &ToolEntry,
    ) -> Result<EffectOutcome> {
        // `comm=` prints the executable name only: other processes'
        // arguments (which may embed secrets) never enter the output.
        let output = std::process::Command::new("ps")
            .args(["-axo", "pid=,pcpu=,pmem=,comm="])
            .output()?;
        anyhow::ensure!(output.status.success(), "ps failed");
        Ok(EffectOutcome {
            output: capped(String::from_utf8_lossy(&output.stdout).into_owned()),
            mutated: false,
        })
    }
}

struct SysDisk;

#[async_trait::async_trait]
impl CandidateToolHandler for SysDisk {
    async fn apply(
        &self,
        workspace: &CandidateWorkspace,
        call: &perspt_sdk::ProviderToolCall,
        _entry: &ToolEntry,
    ) -> Result<EffectOutcome> {
        let target = match call.arguments.get("path").and_then(|v| v.as_str()) {
            Some(relative) => {
                let relative = workspace.validate_relative(relative)?;
                workspace.overlay_root().join(relative)
            }
            None => workspace.overlay_root().to_path_buf(),
        };
        let stat = rustix::fs::statvfs(&target)?;
        let block = stat.f_frsize.max(1);
        let total = stat.f_blocks * block;
        let available = stat.f_bavail * block;
        Ok(EffectOutcome {
            output: format!(
                "path: {}\ntotal_bytes: {total}\navailable_bytes: {available}",
                target.display()
            ),
            mutated: false,
        })
    }
}

struct SysEnv;

#[async_trait::async_trait]
impl CandidateToolHandler for SysEnv {
    async fn apply(
        &self,
        _workspace: &CandidateWorkspace,
        _call: &perspt_sdk::ProviderToolCall,
        _entry: &ToolEntry,
    ) -> Result<EffectOutcome> {
        let mut names: Vec<String> = std::env::vars_os()
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect();
        names.sort();
        Ok(EffectOutcome {
            output: capped(names.join("\n")),
            mutated: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sys_env_reports_names_without_values() {
        std::env::set_var("PERSPT_TEST_SECRET", "super-secret-value");
        let dir = tempfile::tempdir().unwrap();
        let workspace = CandidateWorkspace::create(dir.path(), "n1", 0, "r1").unwrap();
        let call = perspt_sdk::ProviderToolCall {
            call_id: "c1".into(),
            name: "sys_env".into(),
            arguments: serde_json::json!({}),
        };
        let outcome = SysEnv
            .apply(&workspace, &call, &entries()[3])
            .await
            .unwrap();
        assert!(outcome.output.contains("PERSPT_TEST_SECRET"));
        assert!(!outcome.output.contains("super-secret-value"));
        std::env::remove_var("PERSPT_TEST_SECRET");
    }
}
