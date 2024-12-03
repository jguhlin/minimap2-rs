pub use crate::datatypes::*;

use crossterm::event::KeyEvent;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

pub struct QuerySequencesStore {
    query_sequences: Vec<Arc<QuerySequence>>,
    dispatcher_tx: UnboundedSender<Action>,
    current: usize,
}

impl QuerySequencesStore {
    pub fn new(dispatcher_tx: UnboundedSender<Action>) -> Self {
        Self {
            query_sequences: Vec::new(),
            dispatcher_tx,
            current: 0,
        }
    }

    pub async fn dispatch(&mut self, action: Action) {
        match action {
            Action::AddQuerySequence(query) => {
                self.add_query_sequence(query);
            }
            Action::SetSelectedQuery(n) => {
                self.set_current(n);
                // Set selected mappings
                self.dispatcher_tx
                    .send(Action::SetSelectedMappings(
                        self.query_sequences[self.current].id.clone(),
                    ))
                    .expect("Unable to send selected mappings");
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
    pub fn set_current(&mut self, n: usize) {
        // Clamp
        let n = n.min(self.query_sequences.len() - 1);
        let n = n.max(0);
        self.current = n;
        let _ = self
            .dispatcher_tx
            .send(Action::UpdateUiStateSelectQuerySequence((
                n,
                Arc::clone(&self.query_sequences[n]),
            )));
    }

    pub fn keypress(&self, key: KeyEvent) {
        match key.code {
            // Up arrow
            crossterm::event::KeyCode::Up => {
                log::trace!("Up arrow pressed");
                if self.query_sequences.len() > 0 && self.current > 0 {
                    self.dispatcher_tx
                        .send(Action::SetSelectedQuery(self.current - 1))
                        .expect("Unable to send selected query");
                }
            }
            // Down arrow
            crossterm::event::KeyCode::Down => {
                log::trace!("Down arrow pressed");
                if self.query_sequences.len() > 0 && self.current < self.query_sequences.len() - 1 {
                    self.dispatcher_tx
                        .send(Action::SetSelectedQuery(self.current + 1))
                        .expect("Unable to send selected query");
                }
            }
            _ => {}
        }
    }
}
