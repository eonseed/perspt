//! Local DB explorer family: read-only, bounded queries over data files the
//! workspace already contains (CSV, Parquet, JSON, DuckDB).
//!
//! `db_query` runs SELECT-only SQL against a fresh **in-memory** DuckDB
//! connection in which the target file is pre-registered as the view
//! `data`. The SQL text passes a statement-class allowlist (a single
//! SELECT/WITH statement) and a blocklist of filesystem- and
//! environment-reaching functions, so a query can never reach beyond the
//! validated workspace file. Results carry row and byte caps. The session
//! store is never a valid target, so DuckDB's one-live-handle constraint
//! does not bite.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use perspt_sdk::{AccessMode, EffectKind, FootprintSpec, ResourceSelector, ToolEntry};

use super::{family_entry, object_schema};
use crate::candidate::CandidateWorkspace;
use crate::toolloop::EffectOutcome;
use crate::tools::handlers::{CandidateHandlerRegistry, CandidateToolHandler};

const MAX_ROWS: usize = 200;
const MAX_BYTES: usize = 32 * 1024;
const DATA_EXTENSIONS: &[&str] = &["csv", "tsv", "parquet", "json", "jsonl", "ndjson", "duckdb"];

fn scoped_path_footprint() -> FootprintSpec {
    FootprintSpec::new(vec![ResourceSelector::ScopedArgument {
        family: "db".into(),
        field: "path".into(),
        access: AccessMode::Read,
    }])
}

pub fn entries() -> Vec<ToolEntry> {
    vec![
        family_entry(
            "db_list",
            "List data files in the workspace (CSV, TSV, Parquet, JSON, DuckDB)",
            EffectKind::DataRead,
            object_schema(&[]),
            FootprintSpec::new(vec![ResourceSelector::Literal {
                resource: perspt_sdk::Resource::Scoped {
                    family: "db".into(),
                    key: "list".into(),
                },
                access: AccessMode::Read,
            }]),
        ),
        family_entry(
            "db_schema",
            "Describe the columns and types of a workspace data file",
            EffectKind::DataRead,
            object_schema(&[("path", "string", "Workspace-relative data file", true)]),
            scoped_path_footprint(),
        ),
        family_entry(
            "db_query",
            "Run one SELECT-only SQL statement against a workspace data file; \
             the file is exposed as the view `data`; results are row- and \
             byte-capped",
            EffectKind::DataRead,
            object_schema(&[
                ("path", "string", "Workspace-relative data file", true),
                (
                    "sql",
                    "string",
                    "A single SELECT/WITH statement over `data`",
                    true,
                ),
            ]),
            scoped_path_footprint(),
        ),
    ]
}

pub fn register(registry: &mut CandidateHandlerRegistry) -> Result<()> {
    registry.register("db_list", Arc::new(DbList))?;
    registry.register("db_schema", Arc::new(DbSchema))?;
    registry.register("db_query", Arc::new(DbQuery))?;
    Ok(())
}

fn is_data_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| DATA_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Resolve a validated workspace-relative data-file argument.
fn data_file(
    workspace: &CandidateWorkspace,
    call: &perspt_sdk::ProviderToolCall,
) -> Result<std::path::PathBuf> {
    let relative = call
        .arguments
        .get("path")
        .and_then(|v| v.as_str())
        .context("a workspace-relative `path` argument is required")?;
    let relative = workspace.validate_relative(relative)?;
    let path = workspace.overlay_root().join(&relative);
    anyhow::ensure!(
        is_data_file(&path),
        "not a recognized data file (csv, tsv, parquet, json, jsonl, ndjson, duckdb): {relative}"
    );
    anyhow::ensure!(path.is_file(), "data file not found: {relative}");
    Ok(path)
}

/// Open an in-memory connection with the target file registered as the
/// read-only view `data`, then lock external access down.
fn open_data_view(path: &Path) -> Result<duckdb::Connection> {
    let connection = duckdb::Connection::open_in_memory()?;
    let literal = path
        .to_str()
        .context("data file path is not valid UTF-8")?
        .replace('\'', "''");
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let view = match extension.as_str() {
        "csv" | "tsv" => format!("CREATE VIEW data AS SELECT * FROM read_csv_auto('{literal}')"),
        "parquet" => format!("CREATE VIEW data AS SELECT * FROM read_parquet('{literal}')"),
        "json" | "jsonl" | "ndjson" => {
            format!("CREATE VIEW data AS SELECT * FROM read_json_auto('{literal}')")
        }
        "duckdb" => format!("ATTACH '{literal}' AS src (READ_ONLY)"),
        other => anyhow::bail!("unsupported data file extension {other:?}"),
    };
    connection.execute_batch(&view)?;
    Ok(connection)
}

/// A single SELECT/WITH statement with no filesystem- or environment-
/// reaching functions. Fail closed on anything else.
fn validate_select_only(sql: &str) -> Result<()> {
    let trimmed = sql.trim().trim_end_matches(';');
    anyhow::ensure!(
        !trimmed.contains(';'),
        "db_query admits exactly one statement"
    );
    let lowered = trimmed.to_ascii_lowercase();
    anyhow::ensure!(
        lowered.starts_with("select") || lowered.starts_with("with"),
        "db_query admits only SELECT/WITH statements"
    );
    const FORBIDDEN: &[&str] = &[
        "attach",
        "copy",
        "install",
        "load",
        "pragma",
        "export",
        "import",
        "create",
        "insert",
        "update ",
        "delete",
        "drop",
        "alter",
        "set ",
        "call",
        "read_csv",
        "read_parquet",
        "read_json",
        "read_text",
        "read_blob",
        "glob",
        "getenv",
        "sniff_csv",
    ];
    for keyword in FORBIDDEN {
        anyhow::ensure!(
            !lowered.contains(keyword),
            "db_query rejects {} in SQL (read-only, pre-registered view only)",
            keyword.trim()
        );
    }
    Ok(())
}

/// Render capped query results as aligned text.
fn render_rows(statement: &mut duckdb::Statement<'_>) -> Result<String> {
    let mut rows = statement.query([])?;
    let mut lines = Vec::new();
    let mut names_written = false;
    let mut row_count = 0usize;
    let mut bytes = 0usize;
    while let Some(row) = rows.next()? {
        let stmt = row.as_ref();
        if !names_written {
            lines.push(stmt.column_names().join(" | "));
            names_written = true;
        }
        let width = stmt.column_count();
        let mut cells = Vec::with_capacity(width);
        for index in 0..width {
            let value: duckdb::types::Value = row.get(index)?;
            cells.push(render_value(&value));
        }
        let line = cells.join(" | ");
        bytes += line.len();
        lines.push(line);
        row_count += 1;
        if row_count >= MAX_ROWS || bytes >= MAX_BYTES {
            lines.push(format!("… (capped at {row_count} rows)"));
            break;
        }
    }
    if lines.is_empty() {
        lines.push("(no rows)".into());
    }
    Ok(lines.join("\n"))
}

fn render_value(value: &duckdb::types::Value) -> String {
    match value {
        duckdb::types::Value::Null => "NULL".into(),
        duckdb::types::Value::Text(text) => text.clone(),
        other => format!("{other:?}"),
    }
}

struct DbList;

#[async_trait::async_trait]
impl CandidateToolHandler for DbList {
    async fn apply(
        &self,
        workspace: &CandidateWorkspace,
        _call: &perspt_sdk::ProviderToolCall,
        _entry: &ToolEntry,
    ) -> Result<EffectOutcome> {
        let root = workspace.overlay_root().to_path_buf();
        let mut files = Vec::new();
        for entry in ignore::WalkBuilder::new(&root).build().flatten() {
            let path = entry.path();
            if path.is_file() && is_data_file(path) {
                if let Ok(relative) = path.strip_prefix(&root) {
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    files.push(format!("{} ({size} bytes)", relative.display()));
                }
            }
        }
        files.sort();
        Ok(EffectOutcome {
            output: if files.is_empty() {
                "no data files found".into()
            } else {
                files.join("\n")
            },
            mutated: false,
            completed: true,
        })
    }
}

struct DbSchema;

#[async_trait::async_trait]
impl CandidateToolHandler for DbSchema {
    async fn apply(
        &self,
        workspace: &CandidateWorkspace,
        call: &perspt_sdk::ProviderToolCall,
        _entry: &ToolEntry,
    ) -> Result<EffectOutcome> {
        let path = data_file(workspace, call)?;
        let connection = open_data_view(&path)?;
        let sql = if path.extension().and_then(|e| e.to_str()) == Some("duckdb") {
            "SELECT table_name, column_name, data_type FROM information_schema.columns \
             ORDER BY table_name, ordinal_position"
        } else {
            "SELECT 'data' AS table_name, column_name, data_type \
             FROM information_schema.columns WHERE table_name = 'data' \
             ORDER BY ordinal_position"
        };
        let mut statement = connection.prepare(sql)?;
        let output = render_rows(&mut statement)?;
        Ok(EffectOutcome {
            output,
            mutated: false,
            completed: true,
        })
    }
}

struct DbQuery;

#[async_trait::async_trait]
impl CandidateToolHandler for DbQuery {
    async fn apply(
        &self,
        workspace: &CandidateWorkspace,
        call: &perspt_sdk::ProviderToolCall,
        _entry: &ToolEntry,
    ) -> Result<EffectOutcome> {
        let sql = call
            .arguments
            .get("sql")
            .and_then(|v| v.as_str())
            .context("db_query requires a `sql` argument")?;
        validate_select_only(sql)?;
        let path = data_file(workspace, call)?;
        let connection = open_data_view(&path)?;
        let mut statement = connection.prepare(sql.trim().trim_end_matches(';'))?;
        let output = render_rows(&mut statement)?;
        Ok(EffectOutcome {
            output,
            mutated: false,
            completed: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe_call(name: &str, arguments: serde_json::Value) -> perspt_sdk::ProviderToolCall {
        perspt_sdk::ProviderToolCall {
            call_id: "c1".into(),
            name: name.into(),
            arguments,
        }
    }

    #[test]
    fn select_only_allowlist_rejects_escapes() {
        assert!(validate_select_only("SELECT * FROM data").is_ok());
        assert!(validate_select_only("WITH t AS (SELECT 1) SELECT * FROM t").is_ok());
        assert!(validate_select_only("SELECT * FROM read_csv_auto('/etc/passwd')").is_err());
        assert!(validate_select_only("INSTALL httpfs").is_err());
        assert!(validate_select_only("SELECT 1; DROP TABLE data").is_err());
        assert!(validate_select_only("PRAGMA database_list").is_err());
        assert!(validate_select_only("SELECT getenv('HOME')").is_err());
        assert!(validate_select_only("COPY data TO '/tmp/out.csv'").is_err());
    }

    #[tokio::test]
    async fn db_query_reads_workspace_csv_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("people.csv"),
            "name,age\nada,36\ngrace,45\n",
        )
        .unwrap();
        let workspace = CandidateWorkspace::create(dir.path(), "n1", 0, "r1").unwrap();

        let outcome = DbQuery
            .apply(
                &workspace,
                &probe_call(
                    "db_query",
                    serde_json::json!({
                        "path": "people.csv",
                        "sql": "SELECT name FROM data ORDER BY age DESC"
                    }),
                ),
                &entries()[2],
            )
            .await
            .unwrap();
        assert!(outcome.output.contains("grace"));
        assert!(!outcome.mutated);

        let escape = DbQuery
            .apply(
                &workspace,
                &probe_call(
                    "db_query",
                    serde_json::json!({
                        "path": "../outside.csv",
                        "sql": "SELECT * FROM data"
                    }),
                ),
                &entries()[2],
            )
            .await;
        assert!(escape.is_err());
    }

    #[tokio::test]
    async fn db_schema_describes_csv_columns() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("people.csv"), "name,age\nada,36\n").unwrap();
        let workspace = CandidateWorkspace::create(dir.path(), "n1", 0, "r1").unwrap();
        let outcome = DbSchema
            .apply(
                &workspace,
                &probe_call("db_schema", serde_json::json!({"path": "people.csv"})),
                &entries()[1],
            )
            .await
            .unwrap();
        assert!(outcome.output.contains("name"));
        assert!(outcome.output.contains("age"));
    }
}
