use crossbeam::queue::ArrayQueue;
use minimap2::*;
use needletail::{parse_fastx_file, FastxReader};

use std::path::PathBuf;
use std::{error::Error, path::Path, sync::Arc, time::Duration};

use clap::{Parser, ValueEnum};

/// We use a worker queue to pass around work between threads.
/// We do it this way to be generic over the type.
enum WorkQueue<T> {
    Work(T),
    Result(T),
}

// Not necessary to make types, but it helps keep things straightforward

// We work on distinct WorkUnits (aka a single sequence)
type WorkUnit = (Vec<u8>, Vec<u8>); // Sequence ID, Sequence

// We return the original sequence, and the vector of mappings (results)
type WorkResult = (WorkUnit, Vec<Mapping>);
// We could also choose to return just the Seq ID, and the Mappings should also have the query name too
// You can return whatever would be most useful

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
    // Aligner gets created using the build pattern.
    // Once .with_index is called, the aligner is set to "Built" and can no longer be changed.
    println!("Creating index");
    let aligner = Aligner::builder()
        .map_ont()
        .with_cigar()
        .with_index_threads(threads)
        .with_index(target_file, None)
        .expect("Unable to build index");

    println!("Index created");

    // Create a queue for work and for results
    let work_queue = Arc::new(ArrayQueue::<WorkQueue<WorkUnit>>::new(1024));
    let results_queue = Arc::new(ArrayQueue::<WorkQueue<WorkResult>>::new(1024));

    // I use a shutdown flag
    let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Store join handles, it's just good practice to clean up threads
    let mut jh = Vec::new();

    let aligner = Arc::new(aligner);

    // Spin up the threads
    for _ in 0..threads {
        // Clone everything we will need...
        let work_queue = Arc::clone(&work_queue);
        let results_queue = Arc::clone(&results_queue);
        let shutdown = Arc::clone(&shutdown);
        let aligner = Arc::clone(&aligner);

        let handle =
            std::thread::spawn(move || worker(work_queue, results_queue, shutdown, aligner));

        jh.push(handle);
    }

    // Let's split this into another thread

    {
        let work_queue = Arc::clone(&work_queue);
        let shutdown = Arc::clone(&shutdown);
        let query_file = query_file.as_ref().to_path_buf();

        let handle = std::thread::spawn(move || {
            // Now that the threads are running, read the input file and push the work to the queue
            let mut reader: Box<dyn FastxReader> =
                parse_fastx_file(query_file).unwrap_or_else(|_| panic!("Can't find query FASTA file"));

            // I just do this in the main thread, but you can split threads
            let backoff = crossbeam::utils::Backoff::new();
            while let Some(Ok(record)) = reader.next() {
                let mut work = WorkQueue::Work((record.id().to_vec(), record.seq().to_vec()));
                while let Err(work_packet) = work_queue.push(work) {
                    work = work_packet; // Simple way to maintain ownership
                                        // If we have an error, it's 99% because the queue is full
                    backoff.snooze();
                }
            }

            // Set the shutdown flag
            shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
        });

        jh.push(handle);
    }

    let mut num_alignments = 0;

    let backoff = crossbeam::utils::Backoff::new();
    loop {
        match results_queue.pop() {
            // This is where we processs mapping results as they come in...
            Some(WorkQueue::Result((record, alignments))) => {
                num_alignments += alignments.len();

            }
            Some(_) => unimplemented!("Unexpected result type"),
            None => {
                backoff.snooze();

                // If all join handles are finished, we can break
                if jh.iter().all(|h| h.is_finished()) {
                    break;
                }
                if backoff.is_completed() {
                    backoff.reset();
                    std::thread::sleep(Duration::from_millis(3));
                }
            }
        }
    }

    // Join all the threads
    for handle in jh {
        handle.join().expect("Unable to join thread");
    }

    println!("Total alignments: {}", num_alignments);

    Ok(())
}

// Convert this to a function
fn worker(
    work_queue: Arc<ArrayQueue<WorkQueue<WorkUnit>>>,
    results_queue: Arc<ArrayQueue<WorkQueue<WorkResult>>>,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    aligner: Arc<Aligner<Built>>,
) {
    loop {
        // We use backoff to sleep when we don't have any work
        let backoff = crossbeam::utils::Backoff::new();

        match work_queue.pop() {
            Some(WorkQueue::Work(sequence)) => {
                let alignment = aligner
                    .map(&sequence.1, true, false, None, None, Some(&sequence.0))
                    .expect("Unable to align");

                // Return the original sequence, as well as the mappings
                // We have to do it this way because ownership
                let mut result = WorkQueue::Result((sequence, alignment));
                while let Err(result_packet) = results_queue.push(result) {
                    result = result_packet; // Simple way to maintain ownership
                                            // If we have an error, it's 99% because the queue is full
                    backoff.snooze();
                }
            }
            Some(_) => unimplemented!("Unexpected work type"),
            None => {
                backoff.snooze();

                // If we have the shutdown signal, we should exit
                if shutdown.load(std::sync::atomic::Ordering::Relaxed) && work_queue.is_empty() {
                    break;
                }
            }
        }
    }
}
