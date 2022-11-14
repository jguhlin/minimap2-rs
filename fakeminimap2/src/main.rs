use std::io::{BufReader, Read};
use std::sync::Arc;

use clap::Parser;
use crossbeam::queue::ArrayQueue;
use flate2::read::GzDecoder;
use minimap2::*;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    reference: String,
    query: String,
}

enum WorkQueue<T> {
    Work(T),
    Done,
}

fn main() {
    let cli = Cli::parse();

    let mut aligner = Aligner {
        threads: 8,
        ..map_ont()
    }
    .with_cigar()
    .with_index(&cli.reference, None)
    .expect("Unable to build index");

    let query_file = cli.query;

    // Read the first 50 bytes of the file
    let mut f = std::fs::File::open(&query_file).unwrap();
    let mut buffer = [0; 50];
    f.read(&mut buffer).unwrap();
    // Close the file
    drop(f);

    // Check if the file is gzipped
    let compression_type = detect_compression_format(&buffer).unwrap();
    if compression_type != CompressionType::NONE && compression_type != CompressionType::GZIP {
        panic!("Compression type is not supported");
    }

    // If gzipped, open it with a reader...
    let mut reader: Box<dyn Read> = if compression_type == CompressionType::GZIP {
        Box::new(GzDecoder::new(std::fs::File::open(&query_file).unwrap()))
    } else {
        Box::new(std::fs::File::open(&query_file).unwrap())
    };

    // Check the file type
    let mut buffer = [0; 4];
    reader.read(&mut buffer).unwrap();
    let file_type = detect_file_format(&buffer).unwrap();
    if file_type != FileFormat::FASTA && file_type != FileFormat::FASTQ {
        panic!("File type is not supported");
    }

    let work_queue = Arc::new(ArrayQueue::new(128));
    let results_queue = Arc::new(ArrayQueue::new(128));
    // TODO: Make threads clap argument

    // 8 threads
    for i in 0..8 {
        let work_queue = Arc::clone(&work_queue);
        let results_queue = Arc::clone(&results_queue);

        // This could/should be done in the new thread, but wanting to test out the ability to move...
        let mut aligner = aligner.clone();

        std::thread::spawn(move || loop {
            let backoff = crossbeam::utils::Backoff::new();
            let work = work_queue.pop();
            match work {
                Some(WorkQueue::Work(sequence)) => {
                    println!("Got work");
                    let alignment = aligner
                        .map(&sequence.sequence.unwrap(), false, false, None, None)
                        .expect("Unable to align");
                    println!("Alignment len: {}", alignment.len());
                    results_queue.push(WorkQueue::Work(alignment));
                }
                Some(WorkQueue::Done) => {
                    println!("Got done");
                    results_queue.push(WorkQueue::Done);
                    break;
                }
                None => {
                    backoff.snooze();
                }
            }
        });
    }

    let wq = Arc::clone(&work_queue);
    std::thread::spawn(move || {
        // If gzipped, open it with a reader...
        let reader: Box<dyn Read> = if compression_type == CompressionType::GZIP {
            Box::new(GzDecoder::new(std::fs::File::open(&query_file).unwrap()))
        } else {
            Box::new(std::fs::File::open(query_file).unwrap())
        };

        let mut reader = BufReader::new(reader);

        let mut reader: Box<dyn Iterator<Item = Result<Sequence, &'static str>>> =
            if file_type == FileFormat::FASTA {
                Box::new(Fasta::from_buffer(&mut reader))
            } else {
                Box::new(Fastq::from_buffer(&mut reader))
            };

        for seq in reader {
            let seq = seq.unwrap();
            wq.push(WorkQueue::Work(seq));
        }

        for _ in 0..8 {
            wq.push(WorkQueue::Done);
        }
    });

    loop {
        let result = results_queue.pop();
        match result {
            Some(WorkQueue::Work(alignment)) => {
                println!("{:#?}", alignment);
            },
            Some(WorkQueue::Done) => {
                break;
            },
            None => {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }


            
        }
    }
}
