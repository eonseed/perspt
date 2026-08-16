use super::*;
use std::env::temp_dir;

#[tokio::test]
async fn test_read_file() {
    let dir = temp_dir();
    let test_file = dir.join("test_read.txt");
    fs::write(&test_file, "Hello, World!").unwrap();

    let tools = AgentTools::new(dir.clone(), false);
    let call = ToolCall {
        name: "read_file".to_string(),
        arguments: [("path".to_string(), test_file.to_string_lossy().to_string())]
            .into_iter()
            .collect(),
    };

    let result = tools.execute(&call).await;
    assert!(result.success);
    assert_eq!(result.output, "Hello, World!");
}

#[tokio::test]
async fn test_list_files() {
    let dir = temp_dir();
    let tools = AgentTools::new(dir.clone(), false);
    let call = ToolCall {
        name: "list_files".to_string(),
        arguments: HashMap::new(),
    };

    let result = tools.execute(&call).await;
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

    let tools = AgentTools::new(temp_dir.clone(), true);

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

    let tools = AgentTools::new(dir.clone(), false);
    let mut args = HashMap::new();
    args.insert("path".to_string(), test_file.to_string_lossy().to_string());
    let call = ToolCall {
        name: "delete_file".to_string(),
        arguments: args,
    };
    let result = tools.execute(&call).await;
    assert!(result.success, "Delete should succeed: {:?}", result.error);
    assert!(!test_file.exists(), "File should be gone");
}

#[tokio::test]
async fn test_delete_nonexistent_file_succeeds() {
    let dir = temp_dir();
    let tools = AgentTools::new(dir.clone(), false);
    let mut args = HashMap::new();
    args.insert(
        "path".to_string(),
        "/tmp/does_not_exist_xyz.txt".to_string(),
    );
    let call = ToolCall {
        name: "delete_file".to_string(),
        arguments: args,
    };
    let result = tools.execute(&call).await;
    assert!(result.success);
}

#[tokio::test]
async fn test_move_file() {
    let dir = temp_dir();
    let src = dir.join("test_move_src.txt");
    let dst = dir.join("test_move_dst.txt");
    fs::write(&src, "move me").unwrap();

    let tools = AgentTools::new(dir.clone(), false);
    let mut args = HashMap::new();
    args.insert("to".to_string(), dst.to_string_lossy().to_string());
    // The governed catalog names the source argument `path`.
    args.insert("path".to_string(), src.to_string_lossy().to_string());
    let call = ToolCall {
        name: "move_file".to_string(),
        arguments: args,
    };
    let result = tools.execute(&call).await;
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

    let tools = AgentTools::new(temp_dir(), false);
    let mut args = HashMap::new();
    args.insert("path".to_string(), dir.to_string_lossy().to_string());
    let call = ToolCall {
        name: "delete_file".to_string(),
        arguments: args,
    };
    let result = tools.execute(&call).await;
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

    let tools = AgentTools::new(dir.clone(), false);
    let mut args = HashMap::new();
    args.insert("from".to_string(), src.to_string_lossy().to_string());
    args.insert("to".to_string(), dst.to_string_lossy().to_string());
    args.insert("path".to_string(), src.to_string_lossy().to_string());
    let call = ToolCall {
        name: "move_file".to_string(),
        arguments: args,
    };
    let result = tools.execute(&call).await;
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
