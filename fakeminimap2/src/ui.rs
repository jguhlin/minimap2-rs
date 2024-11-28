use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::state as state;

mod app_display;
use app_display::AppDisplay;

const RENDERING_TICK_RATE: Duration = Duration::from_millis(250);

pub async fn main_loop(
    dispatcher_tx: UnboundedSender<state::Action>,
    mut ui_rx: UnboundedReceiver<state::UiState>,
) {
    // Get the first state
    let mut app_display = {
        let state = ui_rx.recv().await.unwrap();
        AppDisplay::new(state, dispatcher_tx.clone())
    };

    app_display.render();
    

    let mut ticker = tokio::time::interval(RENDERING_TICK_RATE);
    loop {
        tokio::select! {
            _ = ticker.tick() => (),
            Some(state) = ui_rx.recv() => {
                // Update the state
                app_display.set_state(state);
            }
        };

        app_display.render();
    }

}