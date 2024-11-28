use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "fakeminimap2",
    about = "An example of how to use the minimap2 crate with multithreading"
)]
pub(crate) struct Cli {
    /// The target file to align to (e.g. a reference genome - can be in FASTA, FASTQ, or mmi format)
    pub target: PathBuf,

    /// The query file to align (e.g. reads - can be FASTA or FASTQ)
    pub query: PathBuf,

    /// The number of threads to use
    pub threads: usize,

    /// The method to use for multithreading
    pub method: Option<Method>,
}

#[derive(ValueEnum, Debug, Default, Clone)]
pub(crate) enum Method {
    #[default]
    /// Use crossbeam channels for multithreading (default)
    Channels,

    /// Use rayon for multithreading
    Rayon,
}

pub(crate) fn parse_args() -> Cli {
    Cli::parse()
}
