pub use crate::datatypes::*;

use crossterm::event::KeyEvent;
use dashmap::DashMap;
use minimap2::Mapping;
use tokio::sync::{mpsc::UnboundedSender, Mutex};

use std::sync::Arc;

pub struct MappingResultStore {
    dispatcher_tx: UnboundedSender<Action>,
    mappings: DashMap<String, Arc<Mutex<Vec<Mapping>>>>,
}

impl MappingResultStore {
    pub fn new(dispatcher_tx: UnboundedSender<Action>) -> Self {
        Self {
            dispatcher_tx,
            mappings: DashMap::new(),
        }
    }

    pub async fn dispatch(self: Arc<Self>, action: Action) {
        match action {
            Action::SetSelectedMappings(id) => {
                if self.mappings.get(&id).is_none() {
                    self.mappings
                        .insert(id.clone(), Arc::new(Mutex::new(Vec::new())));
                }

                let selected_mappings = Arc::clone(&self.mappings.get(&id).unwrap());
                self.dispatcher_tx
                    .send(Action::UpdateUiStateSelectMappings(selected_mappings))
                    .expect("Unable to send updated UI state");
            }

            Action::AddMappings((id, mappings)) => {
                let s = Arc::clone(&self);

                tokio::spawn(async move { s.add_mappings(id, mappings).await });
            }
            _ => unimplemented!("Action not implemented or invalid for MappingResultStore"),
        }
    }

    pub async fn add_mappings(self: Arc<Self>, id: String, mapping: Vec<Mapping>) {
        log::debug!("Adding mappings for id: {} - {} found", id, mapping.len());
        // If the entry already exists, update the mappings
        if self.mappings.get(&id).is_some() {
            log::trace!("Entry exists - Updating mappings for id: {}", id);
            let selected_mappings = Arc::clone(&self.mappings.get(&id).unwrap());
            tokio::spawn(async move {
                let mut selected_mappings = selected_mappings.lock().await;
                selected_mappings.extend(mapping);
            });
        } else {
            self.mappings
                .insert(id.clone(), Arc::new(Mutex::new(mapping)));
        }
    }

    pub fn keypress(&self, key: KeyEvent) {}
}
