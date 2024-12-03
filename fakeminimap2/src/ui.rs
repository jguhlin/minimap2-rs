use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, EventStream, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_stream::{wrappers::WatchStream, StreamExt};

use std::time::Duration;
use std::time::Instant;

use crate::state;

mod app_display;
use app_display::AppDisplay;

const RENDERING_TICK_RATE: Duration = Duration::from_millis(200);

pub async fn main_loop(
    dispatcher_tx: UnboundedSender<state::Action>,
    mut ui_rx: tokio::sync::watch::Receiver<Option<state::UiState>>,
) {
    ui_rx
        .wait_for(|val| val.is_some())
        .await
        .expect("Unable to get initial UI state");
    // Get the first state
    let mut app_display = {
        let state = ui_rx.borrow();
        AppDisplay::new(state.as_ref().unwrap().clone(), dispatcher_tx.clone())
    };

    let mut terminal = setup_terminal().expect("Unable to setup terminal");
    let mut ticker = tokio::time::interval(RENDERING_TICK_RATE);
    let mut crossterm_events = EventStream::new();

    app_display.prepare(&mut terminal);
    app_display.render(&mut terminal).await;

    let mut last_frame_instant = Instant::now();

    let mut ui_rx = WatchStream::new(ui_rx);

    loop {
        app_display.last_tick = last_frame_instant.elapsed().into();
        last_frame_instant = Instant::now();

        tokio::select! {
            _ = ticker.tick() => {
                app_display.render(&mut terminal).await;
            },
            Some(state) = ui_rx.next() => {
                // Update the state
                app_display.set_state(state.unwrap());
                app_display.render(&mut terminal).await;
            },
            evt = crossterm_events.next() => match evt {
                Some(Ok(Event::Key(key)))  => {
                    app_display.handle_keypress(key).await;
                },
                Some(Ok(Event::Mouse(evt))) => {
                    let col = evt.column;
                    let row = evt.row;

                    match evt.kind {
                        MouseEventKind::Down(_) => {
                            app_display.handle_click(col, row);
                        },
                        MouseEventKind::Up(_) => {
                        },
                        MouseEventKind::Moved => {
                        }
                        _ => (),
                    }
                    // dispatcher_tx.send(state::Action::Mouse(mouse)).expect("Unable to send mouse event");
                },
                None => break, // Ok(Interrupted::UserInt),
                _ => (),
            },
        };

        if app_display.should_quit() {
            break;
        }
    }

    restore_terminal(&mut terminal).expect("Unable to restore terminal");
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>, &'static str> {
    let mut stdout = std::io::stdout();

    enable_raw_mode().expect("Unable to enable raw mode");

    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).expect("Unable to setup terminal");

    Ok(Terminal::new(CrosstermBackend::new(stdout)).expect("Unable to create terminal"))
}

fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<(), &'static str> {
    disable_raw_mode().expect("Unable to disable raw mode");

    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .expect("Unable to restore terminal");

    Ok(terminal.show_cursor().expect("Unable to show cursor"))
}
