/// Fakeminimap2 is an example of how to use the minimap2 crate with multithreading, preferring crossbeam's channels.
/// Although mpsc is also available in the standard library.
///
/// For logging, pass in RUST_LOG=debug or RUST_LOG=trace to see more information. RUST_LOG=info is also supported.
// CLI interface
mod cli;

// Multithreading methods
mod channels; // I prefer using channels over rayon, but rayon is simpler to use
mod rayon;

// UI Stuff
mod ui;
use tokio::sync::mpsc;

// Ignore the tokio stuff, it's just for visualization and interaction!
#[tokio::main]
async fn main() {
    env_logger::init();

    // Parse command line arguments
    let args = cli::parse_args();
    
    // UI Stuff
    let (dispatcher_tx, dispatcher_rx) = mpsc::unbounded_channel::<state::Action>();
    let (ui_tx, ui_rx) = mpsc::unbounded_channel::<state::UiState>();

    {
        let dispatcher_tx = dispatcher_tx.clone();
        let handle = std::thread::spawn(move || {
            match args.method.unwrap_or_default() {
                cli::Method::Channels => {
                    channels::map_with_channels(args.target, args.query, args.threads, dispatcher_tx.clone())
                        .expect("Error mapping with channels");
                }
                cli::Method::Rayon => {
                    rayon::map(args.target, args.query, args.threads, dispatcher_tx.clone()).expect("Error mapping with rayon");
                }
            }
        });
    }

    // Runs the UI Loop
    tokio::join!(
        state::start_dispatcher(dispatcher_tx.clone(), dispatcher_rx, ui_tx),
        ui::main_loop(dispatcher_tx.clone(), ui_rx),
    );
}

mod state;
