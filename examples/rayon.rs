use minimap2::*;
use needletail::{parse_fastx_file, FastxReader};
use rayon::prelude::*;
use clap::Parser;

use std::path::PathBuf;
use std::{error::Error, path::Path};

#[derive(Parser, Debug)]
#[command(
    name = "minimap2-channels-example",
    about = "An example of how to use the minimap2 crate with multithreading"
)]
struct Cli {
    /// The target file to align to (e.g. a reference genome - can be in FASTA, FASTQ, or mmi format)
    pub target: PathBuf,

    /// The query file to align (e.g. reads - can be FASTA or FASTQ)
    pub query: PathBuf,

    /// The number of threads to use
    pub threads: usize,
}

fn main() {
    // Parse command line arguments
    let args = Cli::parse();

    map(args.target, args.query, args.threads).expect("Unable to map");
}

fn map(
    target_file: impl AsRef<Path>,
    query_file: impl AsRef<Path>,
    threads: usize,
) -> Result<(), Box<dyn Error>> {
    // Set the number of threads to use
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .expect("Unable to set number of threads");

    println!("Creating index");

    // Aligner gets created using the build pattern.
    // Once .with_index is called, the aligner is set to "Built" and can no longer be changed.
    let aligner = Aligner::builder()
        .map_ont()
        .with_cigar()
        .with_index_threads(threads) // Minimap2 uses it's own thread pool for index building
        .with_index(target_file, None)
        .expect("Unable to build index");

    println!("Index created");

    // Read in the query file
    let mut reader = parse_fastx_file(query_file)?;

    let mut queries: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    while let Some(record) = reader.next() {
        let record = record.expect("Error reading record");
        queries.push((record.id().to_vec(), record.seq().to_vec()));
    }

    // Map the queries
    let results: Vec<Vec<Mapping>> = queries
        .par_iter()
        .map(|(id, seq)| {
            aligner
                .map(&seq, false, false, None, None, Some(&id))
                .expect("Error mapping")
        })
        .collect();

    // Count total number of alignments
    let total_alignments: usize = results.iter().map(|x| x.len()).sum();
    println!("Iteration complete, total alignments {}", total_alignments);

    Ok(())
}
