#[derive(Debug, Default)]
pub struct AppState {
    backend_version: String,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            backend_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn backend_version(&self) -> &str {
        &self.backend_version
    }
}

