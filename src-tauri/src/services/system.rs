use crate::state::AppState;

pub struct SystemService<'a> {
    state: &'a AppState,
}

impl<'a> SystemService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub fn backend_version(&self) -> String {
        self.state.backend_version().to_string()
    }
}

