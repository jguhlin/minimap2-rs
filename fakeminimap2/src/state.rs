use crossterm::event::KeyCode;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::watch;

use minimap2::Mapping;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub mod mapping_results_store;
pub mod query_sequences_store;
pub mod ui_state;

pub use crate::datatypes::*;
pub use mapping_results_store::MappingResultStore;
pub use query_sequences_store::QuerySequencesStore;
pub use ui_state::{SelectedPanel, UiState};

pub async fn start_dispatcher(
    dispatcher_tx: UnboundedSender<Action>,
    mut dispatcher_rx: UnboundedReceiver<Action>,
    ui_tx: watch::Sender<Option<UiState>>,
) {
    let mut query_store = QuerySequencesStore::new(dispatcher_tx.clone());
    let mut mapping_store = Arc::new(MappingResultStore::new(dispatcher_tx.clone()));
    let mut ui_state = UiState::new(dispatcher_tx.clone());

    // Initial state
    ui_tx
        .send(Some(ui_state.clone()))
        .expect("Unable to send initial state");

    tokio::spawn(async move {
        while let Some(action) = dispatcher_rx.recv().await {
            match action.target_store() {
                TargetStore::QuerySequences => query_store.dispatch(action).await,
                TargetStore::MappingResults => {
                    let mapping_store = Arc::clone(&mapping_store);
                    mapping_store.dispatch(action).await
                }
                TargetStore::UiState => ui_state.dispatch(action).await,
                TargetStore::Ui => {
                    ui_tx
                        .send(Some(ui_state.clone()))
                        .expect("Unable to send UI state");
                }
                TargetStore::Dispatcher => {
                    // Means we have to make the decision ourselves
                    match action {
                        Action::KeyPress(key) => {
                            match key.code {
                                KeyCode::Char('q') => {
                                    let mut ui_state = ui_state.clone();
                                    ui_state.shutdown = true;
                                    ui_tx.send(Some(ui_state)).expect("Unable to send shutdown");
                                }
                                _ => {
                                    // What's the selected panel?
                                    match ui_state.selected_panel {
                                        SelectedPanel::QuerySequences => query_store.keypress(key),
                                        SelectedPanel::Mappings => mapping_store.keypress(key),
                                    }
                                }
                            }
                        }
                        _ => unimplemented!("Action not implemented or invalid for Dispatcher"),
                    }
                }
            }
        }
    });
}
