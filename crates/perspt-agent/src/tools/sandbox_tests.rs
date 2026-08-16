use super::*;
use tempfile::tempdir;

#[test]
fn test_create_sandbox() {
    let dir = tempdir().unwrap();
    let sandbox = create_sandbox(dir.path(), "sess1", "branch1").unwrap();
    assert!(sandbox.exists());
    assert!(sandbox.ends_with("sess1/branch1"));
}

#[test]
fn test_cleanup_sandbox() {
    let dir = tempdir().unwrap();
    let sandbox = create_sandbox(dir.path(), "sess1", "branch1").unwrap();
    assert!(sandbox.exists());
    cleanup_sandbox(&sandbox).unwrap();
    assert!(!sandbox.exists());
}

#[test]
fn test_cleanup_session_sandboxes() {
    let dir = tempdir().unwrap();
    create_sandbox(dir.path(), "sess1", "b1").unwrap();
    create_sandbox(dir.path(), "sess1", "b2").unwrap();
    let session_dir = dir.path().join(".perspt").join("sandboxes").join("sess1");
    assert!(session_dir.exists());
    cleanup_session_sandboxes(dir.path(), "sess1").unwrap();
    assert!(!session_dir.exists());
}

#[test]
fn test_copy_to_sandbox() {
    let dir = tempdir().unwrap();
    // Create a source file
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();

    let sandbox = create_sandbox(dir.path(), "sess1", "b1").unwrap();
    copy_to_sandbox(dir.path(), &sandbox, "src/main.rs").unwrap();

    let copied = sandbox.join("src/main.rs");
    assert!(copied.exists());
    assert_eq!(fs::read_to_string(copied).unwrap(), "fn main() {}");
}

#[test]
fn test_copy_from_sandbox() {
    let dir = tempdir().unwrap();
    let sandbox = create_sandbox(dir.path(), "sess1", "b1").unwrap();

    // Create a file inside the sandbox
    let sandbox_src = sandbox.join("out");
    fs::create_dir_all(&sandbox_src).unwrap();
    fs::write(sandbox_src.join("result.txt"), "hello").unwrap();

    // Copy back to live workspace
    copy_from_sandbox(&sandbox, dir.path(), "out/result.txt").unwrap();

    let dest = dir.path().join("out/result.txt");
    assert!(dest.exists());
    assert_eq!(fs::read_to_string(dest).unwrap(), "hello");
}

#[test]
fn test_list_sandbox_files_empty() {
    let dir = tempdir().unwrap();
    let sandbox = create_sandbox(dir.path(), "sess1", "b1").unwrap();
    let files = list_sandbox_files(&sandbox).unwrap();
    assert!(files.is_empty());
}

#[test]
fn test_list_sandbox_files_nested() {
    let dir = tempdir().unwrap();
    let sandbox = create_sandbox(dir.path(), "sess1", "b1").unwrap();

    // Create nested structure
    let nested = sandbox.join("a/b");
    fs::create_dir_all(&nested).unwrap();
    fs::write(sandbox.join("top.txt"), "x").unwrap();
    fs::write(nested.join("deep.txt"), "y").unwrap();

    let mut files = list_sandbox_files(&sandbox).unwrap();
    files.sort();
    assert_eq!(files, vec!["a/b/deep.txt", "top.txt"]);
}
