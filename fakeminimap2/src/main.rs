use std::io::{Read, BufReader};

use clap::Parser;
use minimap2::*;
use flate2::read::GzDecoder;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    reference: String,
    query: String,
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
        let alignment = aligner
            .map(&seq.sequence.unwrap(), false, false, None, None)
            .expect("Unable to align");
        println!("{:?}", alignment);
    }
}
