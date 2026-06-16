use rustyrag_adapters::RagService;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub rag: Arc<RagService>,
}

impl AppState {
    pub fn new(rag: RagService) -> Self {
        Self {
            rag: Arc::new(rag),
        }
    }
}
