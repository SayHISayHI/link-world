/// Pipeline status only. Sorting is recorded in `triage_status` and archiving is a
/// Collection-level action, so there is no `triaged` / `archived` variant here
/// (ADR-0009). Persisted values are still plain TEXT columns; this enum is the
/// reference list of states the service layer produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectLifecycle {
    Captured,
    Parsed,
    Enriched,
    Evaluated,
    Deleted,
    Failed,
}
