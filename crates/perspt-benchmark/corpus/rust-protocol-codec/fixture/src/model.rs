#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub kind: String,
    pub sequence: u64,
    pub payload: String,
}
