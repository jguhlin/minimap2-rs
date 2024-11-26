/// Fakeminimap2 is an example of how to use the minimap2 crate with multithreading, preferring crossbeam's channels.
/// Although mpsc is also available in the standard library.
///
/// For logging, pass in RUST_LOG=debug or RUST_LOG=trace to see more information. RUST_LOG=info is also supported.

// CLI interface
mod cli;

// Multithreading methods
mod channels; // I prefer using channels over rayon, but rayon is simpler to use
mod rayon;

fn main() {
    env_logger::init();

    let args = cli::parse_args();

    match args.method.unwrap_or_default() {
        cli::Method::Channels => {
            channels::map_with_channels(args.target, args.query, args.threads).expect("Error mapping with channels");
        }
        cli::Method::Rayon => {
            rayon::map(args.target, args.query, args.threads).expect("Error mapping with rayon");
        }
    }
}
