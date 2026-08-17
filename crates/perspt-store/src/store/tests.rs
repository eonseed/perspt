use super::*;

fn seed_session(store: &SessionStore, session_id: &str) {
    let record = SessionRecord {
        session_id: session_id.to_string(),
        task: "test task".to_string(),
        working_dir: "/tmp/test".to_string(),
        merkle_root: None,
        detected_toolchain: None,
        status: "RUNNING".to_string(),
    };
    store.create_session(&record).unwrap();
}

#[test]
fn test_read_only_store_queries_work() {
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join(format!("perspt_ro_test_{}.db", uuid::Uuid::new_v4()));

    // Create and seed a normal store
    {
        let store = SessionStore::open(&db_path).unwrap();
        seed_session(&store, "ro-test");
    }

    // Open read-only and verify queries work
    let ro = SessionStore::open_read_only(&db_path).unwrap();
    let sessions = ro.list_recent_sessions(10).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, "ro-test");
}

#[test]
fn test_read_only_store_rejects_writes() {
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join(format!("perspt_ro_wr_{}.db", uuid::Uuid::new_v4()));

    // Create the DB first
    {
        let _store = SessionStore::open(&db_path).unwrap();
    }

    // Open read-only and verify writes fail
    let ro = SessionStore::open_read_only(&db_path).unwrap();
    let record = SessionRecord {
        session_id: "should-fail".to_string(),
        task: "test".to_string(),
        working_dir: "/tmp".to_string(),
        merkle_root: None,
        detected_toolchain: None,
        status: "RUNNING".to_string(),
    };
    assert!(ro.create_session(&record).is_err());
}

// =================================================================
// PSP-7: SRBN step records and correction attempts round-trip tests
// =================================================================
