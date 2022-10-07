use clap::Parser;
use minimap2::*;

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
    .with_index(&cli.reference, None).expect("Unable to build index");

    let mappings = aligner.map_file(&cli.query, false, false).expect("Unable to map file");
    for mapping in mappings {
        println!("{:#?}", mapping);
    }
}
