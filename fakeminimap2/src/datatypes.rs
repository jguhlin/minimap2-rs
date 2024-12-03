use crossterm::event::KeyEvent;
use minimap2::Mapping;
use tokio::sync::Mutex;

use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct QuerySequence {
    pub id: String,
    pub sequence: Vec<u8>,
}

impl QuerySequence {
    pub fn new(id: String, sequence: Vec<u8>) -> Self {
        Self { id, sequence }
    }
}
pub enum Action {
    // QueryStore
    AddQuerySequence(QuerySequence),
    SetSelectedQuery(usize), // Will later call: UpdateUiStateSelectQuerySequence

    // Mapping Store (todo)
    SetSelectedMappings(String), // Will later call: UpdateUiStateSelectMappings
    AddMappings((String, Vec<Mapping>)),

    // UI State
    UpdateUiStateSelectQuerySequence((usize, Arc<QuerySequence>)),
    UpdateUiStateSelectMappings(Arc<Mutex<Vec<Mapping>>>),
    UpdateUiStateSelectMapping(usize),
    UpdateUiStateQuerySequencesList(Vec<Arc<QuerySequence>>),
    SetStatus(String),

    // Pass back to renderer
    UpdatedUiState,

    // Key Press
    KeyPress(KeyEvent),
}
pub enum TargetStore {
    QuerySequences,
    MappingResults,
    UiState,
    Ui,
    Dispatcher,
}

impl Action {
    pub fn target_store(&self) -> TargetStore {
        match self {
            Action::AddQuerySequence(_) | Action::SetSelectedQuery(_) => {
                TargetStore::QuerySequences
            }

            Action::SetSelectedMappings(_) | Action::AddMappings(_) => TargetStore::MappingResults,

            Action::UpdateUiStateSelectQuerySequence(_)
            | Action::UpdateUiStateSelectMappings(_)
            | Action::UpdateUiStateSelectMapping(_)
            | Action::SetStatus(_)
            | Action::UpdateUiStateQuerySequencesList(_) => TargetStore::UiState,

            Action::UpdatedUiState => TargetStore::Ui,

            Action::KeyPress(_) => TargetStore::Dispatcher,
        }
    }
}
