use perspt_store::ProvisionalBranchRow;

/// View model for the sandbox monitoring page
pub struct SandboxViewModel {
    pub session_id: String,
    pub branches: Vec<SandboxBranch>,
}

/// A single provisional branch with its sandbox state
pub struct SandboxBranch {
    pub branch_id: String,
    pub node_id: String,
    pub parent_node_id: String,
    pub state: String,
    pub sandbox_dir: Option<String>,
}

impl SandboxViewModel {
    pub fn from_store(session_id: String, rows: Vec<ProvisionalBranchRow>) -> Self {
        let branches = rows
            .into_iter()
            .map(|r| SandboxBranch {
                branch_id: r.branch_id,
                node_id: r.node_id,
                parent_node_id: r.parent_node_id,
                state: r.state,
                sandbox_dir: r.sandbox_dir,
            })
            .collect();
        Self {
            session_id,
            branches,
        }
    }
}
