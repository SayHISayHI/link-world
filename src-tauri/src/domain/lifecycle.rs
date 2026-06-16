#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectLifecycle {
    Captured,
    Parsed,
    Enriched,
    Evaluated,
    Triaged,
    Archived,
    Deleted,
    Failed,
}

