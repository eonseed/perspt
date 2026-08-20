#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Started { job: String },
    Progress(u64),
    Finished,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub job: Option<String>,
    pub progress: u64,
    pub finished: bool,
}
