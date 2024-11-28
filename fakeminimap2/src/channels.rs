use crossbeam::queue::ArrayQueue;
use minimap2::*;
use needletail::{parse_fastx_file, FastxReader};

use std::{error::Error, path::Path, sync::Arc, time::Duration};

use crate::state::QuerySequence;


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

pub(crate) fn map_with_channels(
    target_file: impl AsRef<Path>,
    query_file: impl AsRef<Path>,
    threads: usize,

    // UI Stuff
    dispatcher_tx: tokio::sync::mpsc::UnboundedSender<crate::state::Action>,
) -> Result<(), Box<dyn Error>> {
    // Aligner gets created using the build pattern.
    // Once .with_index is called, the aligner is set to "Built" and can no longer be changed.
    let aligner = Aligner::builder()
        .map_ont()
        .with_cigar()
        .with_index(target_file, None)
        .expect("Unable to build index");

    log::info!("Made aligner");
    // Create a queue for work and for results
    let work_queue = Arc::new(ArrayQueue::<WorkQueue<WorkUnit>>::new(32)); // Artificially low, but the best depends on tuning
    let results_queue = Arc::new(ArrayQueue::<WorkQueue<WorkResult>>::new(32));

    // I use a shutdown flag
    let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Store join handles, it's just good practice to clean up threads
    let mut jh = Vec::new();

    let aligner = Arc::new(aligner);

    // Spin up the threads
    log::info!("Spawn threads");
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

        // Oh and add it to the UI (this should be first, but trying to keep multithreading and UI separate)
        let _ = dispatcher_tx.send(crate::state::Action::AddQuerySequence(
            QuerySequence::new(std::str::from_utf8(record.id()).unwrap().to_string(), record.seq().to_vec()),
        ));
        println!("Sent query sequence");
    }

    // Set the shutdown flag
    shutdown.store(true, std::sync::atomic::Ordering::Relaxed);

    let mut num_alignments = 0;

    let backoff = crossbeam::utils::Backoff::new();
    loop {
        match results_queue.pop() {
            Some(WorkQueue::Result((record, alignments))) => {
                num_alignments += alignments.len();
                log::info!(
                    "Got {} alignments for {}",
                    alignments.len(),
                    std::str::from_utf8(&record.0).unwrap()
                );
            }
            Some(_) => unimplemented!("Unexpected result type"),
            None => {
                log::trace!("Awaiting results");
                backoff.snooze();

                // If all join handles are 'finished' we can break
                if jh.iter().all(|h| h.is_finished()) {
                    break;
                }
                if backoff.is_completed() {
                    backoff.reset();
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
    }

    println!("Iteration complete, total alignments {}", num_alignments);

    // Join all threads
    for handle in jh {
        match handle.join() {
            Ok(_) => log::trace!("Thread finished"),
            Err(e) => log::error!("Thread panicked: {:?}", e),
        }
    }

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
