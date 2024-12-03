pub use crate::datatypes::*;

use minimap2::Mapping;
use tokio::sync::{mpsc::UnboundedSender, Mutex};

use std::sync::Arc;

#[derive(Default, Clone, PartialEq, Eq)]
pub enum SelectedPanel {
    #[default]
    QuerySequences,
    Mappings,
}

#[derive(Clone)]
pub struct UiState {
    pub query_sequences_list: Vec<Arc<QuerySequence>>,
    pub selected_query_sequence: Option<Arc<QuerySequence>>,
    pub mappings: Option<Arc<Mutex<Vec<Mapping>>>>,
    pub selected_panel: SelectedPanel,
    pub shutdown: bool,
    pub status: String,

    dispatcher_tx: UnboundedSender<Action>,
}

impl UiState {
    pub fn new(dispatcher_tx: UnboundedSender<Action>) -> Self {
        Self {
            query_sequences_list: Vec::new(),
            selected_query_sequence: None,
            mappings: None,
            shutdown: false,
            selected_panel: SelectedPanel::default(),
            status: "Starting up!".to_string(),
            dispatcher_tx,
        }
    }

    pub async fn dispatch(&mut self, action: Action) {
        match action {
            Action::UpdateUiStateSelectQuerySequence((n, query)) => {
                self.selected_query_sequence = Some(query);
            }
            Action::UpdateUiStateSelectMapping(n) => {
                self.dispatcher_tx
                    .send(Action::UpdateUiStateSelectMapping(n))
                    .expect("Unable to send updated UI state");
            }
            Action::UpdateUiStateQuerySequencesList(list) => {
                self.query_sequences_list = list;
            }
            Action::UpdateUiStateSelectMappings(mappings) => {
                self.mappings = Some(mappings);
            }
            Action::SetStatus(status) => {
                self.status = status;
                // Send an updated UI state
                self.dispatcher_tx
                    .send(Action::UpdatedUiState)
                    .expect("Unable to send updated UI state");
            }
            _ => unimplemented!("Action not implemented or invalid for UiState"),
        }

        // Sending UpdatedUiState action
        self.dispatcher_tx
            .send(Action::UpdatedUiState)
            .expect("Unable to send updated UI state");
    }
}
