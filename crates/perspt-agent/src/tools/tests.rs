use super::*;
use std::env::temp_dir;

#[tokio::test]
async fn test_read_file() {
    let dir = temp_dir();
    let test_file = dir.join("test_read.txt");
    fs::write(&test_file, "Hello, World!").unwrap();

    let tools = AgentTools::new(dir.clone());
    let call = ToolCall {
        name: "read_file".to_string(),
        arguments: [("path".to_string(), test_file.to_string_lossy().to_string())]
            .into_iter()
            .collect(),
    };

    let result = tools.read_file(&call);
    assert!(result.success);
    assert!(result.output.contains("lines 1-1 of 1"));
    assert!(result.output.contains("     1\tHello, World!"));
    assert!(!result.output.contains("more lines"));
}

fn read_call(path: &std::path::Path, extra: &[(&str, &str)]) -> ToolCall {
    let mut arguments: HashMap<String, String> =
        [("path".to_string(), path.to_string_lossy().to_string())]
            .into_iter()
            .collect();
    for (k, v) in extra {
        arguments.insert((*k).to_string(), (*v).to_string());
    }
    ToolCall {
        name: "read_file".to_string(),
        arguments,
    }
}

#[tokio::test]
async fn read_file_pages_with_offset_limit_and_continuation_hint() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("paged.txt");
    let body: String = (1..=10).map(|i| format!("line {i}\n")).collect();
    fs::write(&file, body).unwrap();
    let tools = AgentTools::new(dir.path().to_path_buf());

    let result = tools.read_file(&read_call(&file, &[("offset", "3"), ("limit", "4")]));
    assert!(result.success);
    assert!(result.output.contains("lines 3-6 of 10"));
    assert!(result.output.contains("     3\tline 3"));
    assert!(result.output.contains("     6\tline 6"));
    assert!(!result.output.contains("line 7"));
    assert!(result
        .output
        .contains("[4 more lines; continue with offset=7]"));

    let tail = tools.read_file(&read_call(&file, &[("offset", "7")]));
    assert!(tail.success);
    assert!(tail.output.contains("lines 7-10 of 10"));
    assert!(!tail.output.contains("more lines"));
}

#[tokio::test]
async fn read_file_offset_past_end_reports_total() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("short.txt");
    fs::write(&file, "one\ntwo\n").unwrap();
    let tools = AgentTools::new(dir.path().to_path_buf());

    let result = tools.read_file(&read_call(&file, &[("offset", "9")]));
    assert!(result.success);
    assert!(result.output.contains("2 lines total"));
    assert!(result.output.contains("offset 9 is past the end"));
}

#[tokio::test]
async fn read_file_truncates_very_long_lines() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("long.txt");
    fs::write(&file, "x".repeat(5000)).unwrap();
    let tools = AgentTools::new(dir.path().to_path_buf());

    let result = tools.read_file(&read_call(&file, &[]));
    assert!(result.success);
    assert!(result.output.contains('…'));
    assert!(result.output.len() < 3000);
}

/// Streaming boundedness: an overlong line keeps only its capped prefix
/// (with the omitted byte count), a huge `limit` clamps, and the total is
/// still exact — the file is never staged whole in the window.
#[tokio::test]
async fn read_file_bounds_overlong_lines_and_clamps_the_limit() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("wide.txt");
    let mut body = String::from("short first line\n");
    body.push_str(&"y".repeat(50_000));
    body.push('\n');
    body.push_str("short last line\n");
    fs::write(&file, body).unwrap();
    let tools = AgentTools::new(dir.path().to_path_buf());

    let result = tools.read_file(&read_call(&file, &[("limit", "999999999")]));
    assert!(result.success);
    assert!(result.output.contains("lines 1-3 of 3"));
    assert!(result.output.contains("bytes on this line]"));
    assert!(result.output.contains("overlong lines truncated"));
    assert!(result.output.contains("short last line"));
    assert!(
        result.output.len() < 20_000,
        "the 50KB line must not be returned whole"
    );
}

/// CRLF endings render without the carriage return, and a binary file is
/// refused instead of streamed.
#[tokio::test]
async fn read_file_strips_crlf_and_refuses_binary() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("crlf.txt");
    fs::write(&file, "one\r\ntwo\r\n").unwrap();
    let tools = AgentTools::new(dir.path().to_path_buf());
    let result = tools.read_file(&read_call(&file, &[]));
    assert!(result.success);
    assert!(result.output.contains("     1\tone\n"));
    assert!(!result.output.contains('\r'));

    let binary = dir.path().join("blob.bin");
    fs::write(&binary, b"ab\x00cd\n").unwrap();
    let refused = tools.read_file(&read_call(&binary, &[]));
    assert!(!refused.success);
    assert!(refused.error.unwrap_or_default().contains("binary"));
}

fn search_call(args: &[(&str, &str)]) -> ToolCall {
    ToolCall {
        name: "grep".to_string(),
        arguments: args
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect(),
    }
}

#[tokio::test]
async fn search_code_matches_file_targets_with_relative_paths() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        "fn alpha() {}\nfn beta() {}\n",
    )
    .unwrap();
    let tools = AgentTools::new(dir.path().to_path_buf());

    let result = tools.search_code(&search_call(&[("query", "beta"), ("path", "src/lib.rs")]));
    assert!(result.success, "{:?}", result.error);
    assert!(result.output.contains("src/lib.rs"));
    assert!(result.output.contains("2:"));
    assert!(!result.output.contains("{\"type\""), "must not be rg JSON");
}

#[tokio::test]
async fn search_code_honors_context_and_reports_no_matches() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "before\nneedle\nafter\n").unwrap();
    let tools = AgentTools::new(dir.path().to_path_buf());

    let hit = tools.search_code(&search_call(&[("query", "needle"), ("context", "1")]));
    assert!(hit.success);
    assert!(hit.output.contains("before"));
    assert!(hit.output.contains("after"));

    let miss = tools.search_code(&search_call(&[("query", "absent_token")]));
    assert!(miss.success);
    assert!(miss.output.contains("no matches"));

    let bad_target = tools.search_code(&search_call(&[("query", "x"), ("path", "no/such/dir")]));
    assert!(!bad_target.success);
}

#[tokio::test]
async fn search_code_caps_output_lines() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..30 {
        let body: String = (0..20).map(|j| format!("needle {i} {j}\n")).collect();
        fs::write(dir.path().join(format!("f{i}.txt")), body).unwrap();
    }
    let tools = AgentTools::new(dir.path().to_path_buf());

    let result = tools.search_code(&search_call(&[("query", "needle")]));
    assert!(result.success);
    assert!(result.output.lines().count() <= 201);
    assert!(result.output.contains("more lines omitted"));
}

#[tokio::test]
async fn list_files_respects_gitignore_and_paginates() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".gitignore"), "ignored.log\n").unwrap();
    fs::write(dir.path().join("ignored.log"), "x").unwrap();
    for c in ["alpha.rs", "beta.rs", "gamma.rs"] {
        fs::write(dir.path().join(c), "x").unwrap();
    }
    fs::create_dir(dir.path().join("subdir")).unwrap();
    let tools = AgentTools::new(dir.path().to_path_buf());

    let all = tools.list_files(&ToolCall {
        name: "list_files".to_string(),
        arguments: HashMap::new(),
    });
    assert!(all.success);
    assert!(
        !all.output.contains("ignored.log"),
        "gitignored entry leaked"
    );
    assert!(all.output.contains("alpha.rs"));
    assert!(all.output.contains("subdir/"));

    let page = tools.list_files(&ToolCall {
        name: "list_files".to_string(),
        arguments: [
            ("limit".to_string(), "2".to_string()),
            ("offset".to_string(), "0".to_string()),
        ]
        .into_iter()
        .collect(),
    });
    assert!(page.success);
    assert!(page.output.contains("more entries; continue with offset=2"));
}

#[tokio::test]
async fn glob_paginates_with_continuation_hint() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..5 {
        fs::write(dir.path().join(format!("m{i}.rs")), "x").unwrap();
    }
    let tools = AgentTools::new(dir.path().to_path_buf());

    let page = tools.glob(&ToolCall {
        name: "glob".to_string(),
        arguments: [
            ("pattern".to_string(), "*.rs".to_string()),
            ("limit".to_string(), "3".to_string()),
        ]
        .into_iter()
        .collect(),
    });
    assert!(page.success);
    assert!(page.output.contains("of 5"));
    assert!(page
        .output
        .contains("[2 more entries; continue with offset=3]"));

    let rest = tools.glob(&ToolCall {
        name: "glob".to_string(),
        arguments: [
            ("pattern".to_string(), "*.rs".to_string()),
            ("offset".to_string(), "3".to_string()),
        ]
        .into_iter()
        .collect(),
    });
    assert!(rest.success);
    assert!(rest.output.contains("entries 3-4 of 5"));
    assert!(!rest.output.contains("more entries"));
}

#[tokio::test]
async fn read_file_rejects_malformed_offset() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("f.txt");
    fs::write(&file, "a\n").unwrap();
    let tools = AgentTools::new(dir.path().to_path_buf());

    let bad = tools.read_file(&read_call(&file, &[("offset", "abc")]));
    assert!(!bad.success);
    assert!(bad.error.unwrap().contains("offset"));
    let zero = tools.read_file(&read_call(&file, &[("offset", "0")]));
    assert!(!zero.success);
}

#[tokio::test]
async fn test_list_files() {
    let dir = temp_dir();
    let tools = AgentTools::new(dir.clone());
    let call = ToolCall {
        name: "list_files".to_string(),
        arguments: HashMap::new(),
    };

    let result = tools.list_files(&call);
    assert!(result.success);
}

#[tokio::test]
async fn test_apply_diff_tool() {
    use std::collections::HashMap;
    use std::io::Write;
    let temp_dir = temp_dir();
    let file_path = temp_dir.join("test_diff.txt");
    let mut file = std::fs::File::create(&file_path).unwrap();
    // Explicitly write bytes with unix newlines
    file.write_all(b"Hello world\nThis is a test\n").unwrap();

    let tools = AgentTools::new(temp_dir.clone());

    // Exact string with newlines
    let diff =
        "--- test_diff.txt\n+++ test_diff.txt\n@@ -1,2 +1,2 @@\n-Hello world\n+Hello diffy\n \
        This is a test\n";

    let mut args = HashMap::new();
    args.insert("path".to_string(), "test_diff.txt".to_string());
    args.insert("diff".to_string(), diff.to_string());

    let call = ToolCall {
        name: "apply_diff".to_string(),
        arguments: args,
    };

    let result = tools.apply_diff(&call);
    assert!(
        result.success,
        "Diff application failed: {:?}",
        result.error
    );

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "Hello diffy\nThis is a test\n");
}

#[tokio::test]
async fn test_delete_file() {
    let dir = temp_dir();
    let test_file = dir.join("test_delete_me.txt");
    fs::write(&test_file, "temporary").unwrap();
    assert!(test_file.exists());

    let tools = AgentTools::new(dir.clone());
    let mut args = HashMap::new();
    args.insert("path".to_string(), test_file.to_string_lossy().to_string());
    let call = ToolCall {
        name: "delete_file".to_string(),
        arguments: args,
    };
    let result = tools.delete_file(&call);
    assert!(result.success, "Delete should succeed: {:?}", result.error);
    assert!(!test_file.exists(), "File should be gone");
}

#[tokio::test]
async fn test_delete_nonexistent_file_succeeds() {
    let dir = temp_dir();
    let tools = AgentTools::new(dir.clone());
    let mut args = HashMap::new();
    args.insert(
        "path".to_string(),
        "/tmp/does_not_exist_xyz.txt".to_string(),
    );
    let call = ToolCall {
        name: "delete_file".to_string(),
        arguments: args,
    };
    let result = tools.delete_file(&call);
    assert!(result.success);
}

#[tokio::test]
async fn test_move_file() {
    let dir = temp_dir();
    let src = dir.join("test_move_src.txt");
    let dst = dir.join("test_move_dst.txt");
    fs::write(&src, "move me").unwrap();

    let tools = AgentTools::new(dir.clone());
    let mut args = HashMap::new();
    args.insert("to".to_string(), dst.to_string_lossy().to_string());
    // The governed catalog names the source argument `path`.
    args.insert("path".to_string(), src.to_string_lossy().to_string());
    let call = ToolCall {
        name: "move_file".to_string(),
        arguments: args,
    };
    let result = tools.move_file(&call);
    assert!(result.success, "Move should succeed: {:?}", result.error);
    assert!(!src.exists(), "Source should be gone");
    assert!(dst.exists(), "Destination should exist");
    assert_eq!(fs::read_to_string(&dst).unwrap(), "move me");
    let _ = fs::remove_file(&dst);
}

#[tokio::test]
async fn test_delete_directory_rejected() {
    let dir = temp_dir().join("test_delete_dir");
    fs::create_dir_all(&dir).unwrap();

    let tools = AgentTools::new(temp_dir());
    let mut args = HashMap::new();
    args.insert("path".to_string(), dir.to_string_lossy().to_string());
    let call = ToolCall {
        name: "delete_file".to_string(),
        arguments: args,
    };
    let result = tools.delete_file(&call);
    assert!(!result.success, "Should reject directory deletion");
    let _ = fs::remove_dir(&dir);
}

#[tokio::test]
async fn test_move_file_creates_parent_dirs() {
    let dir = temp_dir();
    let src = dir.join("test_move_nested_src.txt");
    let dst = dir
        .join("nested")
        .join("deep")
        .join("test_move_nested_dst.txt");
    fs::write(&src, "nested move").unwrap();

    let tools = AgentTools::new(dir.clone());
    let mut args = HashMap::new();
    args.insert("from".to_string(), src.to_string_lossy().to_string());
    args.insert("to".to_string(), dst.to_string_lossy().to_string());
    args.insert("path".to_string(), src.to_string_lossy().to_string());
    let call = ToolCall {
        name: "move_file".to_string(),
        arguments: args,
    };
    let result = tools.move_file(&call);
    assert!(
        result.success,
        "Move with nested dirs should succeed: {:?}",
        result.error
    );
    assert!(!src.exists());
    assert!(dst.exists());
    assert_eq!(fs::read_to_string(&dst).unwrap(), "nested move");
    let _ = fs::remove_dir_all(dir.join("nested"));
}
