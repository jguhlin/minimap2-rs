use std::{error::Error, path::Path};

use minimap2::*;
use needletail::{parse_fastx_file, FastxReader};
use rayon::prelude::*;

pub(crate) fn map(
    target_file: impl AsRef<Path>,
    query_file: impl AsRef<Path>,
    threads: usize,

    // UI Stuff
    dispatcher_tx: tokio::sync::mpsc::UnboundedSender<crate::state::Action>,
) -> Result<(), Box<dyn Error>> {
    // Set the number of threads to use
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .expect("Unable to set number of threads");

    // Aligner gets created using the build pattern.
    // Once .with_index is called, the aligner is set to "Built" and can no longer be changed.
    let aligner = Aligner::builder()
        .map_ont()
        .with_cigar()
        .with_index(target_file, None)
        .expect("Unable to build index");

    log::info!("Made aligner");

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

    log::info!("Mapped queries");

    // Count total number of alignments
    let total_alignments: usize = results.iter().map(|x| x.len()).sum();
    println!("Iteration complete, total alignments {}", total_alignments);

    Ok(())
}
