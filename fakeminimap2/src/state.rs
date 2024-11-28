use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::watch;

use minimap2::Mapping;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct UiState {
    pub query_sequences_list: Vec<Arc<QuerySequence>>,
    pub selected_query_sequence: Option<Arc<QuerySequence>>,
    pub mappings: Vec<Mapping>,
    dispatcher_tx: UnboundedSender<Action>,
}

impl UiState {
    pub fn new(dispatcher_tx: UnboundedSender<Action>) -> Self {
        Self {
            query_sequences_list: Vec::new(),
            selected_query_sequence: None,
            mappings: Vec::new(),
            dispatcher_tx,
        }
    }

    pub fn dispatch(&mut self, action: Action) {
        match action {
            Action::UpdateUiStateSelectQuerySequence((n, query)) => {
                self.selected_query_sequence = Some(query);
            }
            Action::UpdateUiStateSelectMapping(n) => {
                self.dispatcher_tx
                    .send(Action::UpdateUiStateSelectMapping(n)).expect("Unable to send updated UI state");
            }
            Action::UpdateUiStateQuerySequencesList(list) => {
                self.query_sequences_list = list;
            }
            _ => unimplemented!("Action not implemented or invalid for UiState"),
        }

        // Sending UpdatedUiState action
        self.dispatcher_tx
            .send(Action::UpdatedUiState).expect("Unable to send updated UI state");
    }
}

#[derive(Debug, Clone)]
pub struct QuerySequence {
    pub id: String,
    pub sequence: Vec<u8>,
}

impl QuerySequence {
    pub fn new(id: String, sequence: Vec<u8>) -> Self {
        Self { id, sequence }
    }
}

pub struct QuerySequencesStore {
    query_sequences: Vec<Arc<QuerySequence>>,
    dispatcher_tx: UnboundedSender<Action>,
}

impl QuerySequencesStore {
    pub fn new(dispatcher_tx: UnboundedSender<Action>) -> Self {
        Self {
            query_sequences: Vec::new(),
            dispatcher_tx,
        }
    }

    pub fn dispatch(&mut self, action: Action) {
        match action {
            Action::AddQuerySequence(query) => {
                self.add_query_sequence(query);
            }
            Action::SetSelectedQuery(n) => {
                self.set_current(n);
            }
            _ => unimplemented!("Action not implemented or invalid for QuerySequencesStore"),
        }
    }

    /// Add a new query sequence
    pub fn add_query_sequence(&mut self, query: QuerySequence) {
        self.query_sequences.push(Arc::new(query));
        self.dispatcher_tx
            .send(Action::UpdateUiStateQuerySequencesList(
                self.query_sequences
                    .iter()
                    .map(|q| Arc::clone(&q))
                    .collect(),
            ))
            .expect("Unable to send updated UI state");
    }

    /// Set the currently viewed QuerySequence
    pub fn set_current(&self, n: usize) {
        let _ = self
            .dispatcher_tx
            .send(Action::UpdateUiStateSelectQuerySequence((
                n,
                Arc::clone(&self.query_sequences[n]),
            )));
    }
}

pub struct MappingResultStore {
    state: Arc<Mutex<HashMap<String, Vec<Mapping>>>>,
    sender: watch::Sender<(String, Vec<Mapping>)>,
    highlighted: watch::Sender<Option<(String, usize)>>, // Tracks highlighted result (Query ID, Index)
}

impl MappingResultStore {
    pub fn new() -> Self {
        let (sender, _) = watch::channel((String::new(), Vec::new()));
        let (highlighted, _) = watch::channel(None);
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            sender,
            highlighted,
        }
    }

    /// Add mapping results and notify subscribers
    pub fn add_mapping_results(&self, id: String, mappings: Vec<Mapping>) {
        let mut state = self.state.lock().unwrap();
        state.insert(id.clone(), mappings.clone());
        let _ = self.sender.send((id, mappings));
    }

    /// Highlight a specific mapping result
    pub fn set_highlighted(&self, id: String, index: usize) {
        let _ = self.highlighted.send(Some((id, index)));
    }

    /// Subscribe to the highlighted result
    pub fn subscribe_highlighted(&self) -> watch::Receiver<Option<(String, usize)>> {
        self.highlighted.subscribe()
    }

    /// Get the current state
    pub fn get_state(&self) -> HashMap<String, Vec<Mapping>> {
        self.state.lock().unwrap().clone()
    }
}

pub enum Action {
    // QueryStore
    AddQuerySequence(QuerySequence),
    SetSelectedQuery(usize), // Will later call: UpdateUiStateSelectQuerySequence

    // Mapping Store (todo)
    SetSelectedMapping(usize), // Will later call: UpdateUiStateSelectMapping

    // UI State
    UpdateUiStateSelectQuerySequence((usize, Arc<QuerySequence>)),
    UpdateUiStateSelectMapping(usize),
    UpdateUiStateQuerySequencesList(Vec<Arc<QuerySequence>>),

    // Pass back to renderer
    UpdatedUiState,
}

enum TargetStore {
    QuerySequences,
    MappingResults,
    UiState,
    Ui,
}

impl Action {
    pub fn target_store(&self) -> TargetStore {
        match self {
            Action::AddQuerySequence(_) | Action::SetSelectedQuery(_) => TargetStore::QuerySequences,
            Action::SetSelectedMapping(_) => TargetStore::MappingResults,
            Action::UpdateUiStateSelectQuerySequence(_)
            | Action::UpdateUiStateSelectMapping(_)
            | Action::UpdateUiStateQuerySequencesList(_) => TargetStore::UiState,
            Action::UpdatedUiState => TargetStore::Ui,
        }
    }
}

pub struct Dispatcher {
    sender: UnboundedSender<Action>,
}

impl Dispatcher {
    pub fn new() -> (Self, UnboundedReceiver<Action>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { sender }, receiver)
    }

    /// Dispatch an action
    pub fn dispatch(&self, action: Action) {
        let _ = self.sender.send(action);
    }
}

pub async fn start_dispatcher(
    dispatcher_tx: UnboundedSender<Action>,
    mut dispatcher_rx: UnboundedReceiver<Action>,
    ui_tx: UnboundedSender<UiState>,
) {

    let mut query_store = QuerySequencesStore::new(dispatcher_tx.clone());
    let mut mapping_store = MappingResultStore::new();
    let mut ui_state = UiState::new(dispatcher_tx.clone());

    // Initial state
    ui_tx.send(ui_state.clone()).expect("Unable to send initial state");

    tokio::spawn(async move {
        while let Some(action) = dispatcher_rx.recv().await {
            match action.target_store() {
                TargetStore::QuerySequences => query_store.dispatch(action),
                // TargetStore::MappingResults => mapping_store.dispatch(action),
                TargetStore::MappingResults => unimplemented!("Soon"),
                TargetStore::UiState => ui_state.dispatch(action),
                TargetStore::Ui => {
                    ui_tx.send(ui_state.clone());
                }
            }
        }
    });
}