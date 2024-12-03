# Multithreaded example of mapping

Using crossbeam and standard library threading, we can multi thread alignments sharing the same references to the index. Small example files are provided.

## Running
Make sure you have [Rustup](https://rustup.rs/) installed.

Git clone the whole repo and go to the fakeminimap2 directory.
```
cargo run --release <TARGET-FASTA> <QUERY-FASTA> <NUM THREADS>
```

Such as
```
cargo run --release Arabidopsis.fna reads.fasta 64
```

You can also 
```
cargo install --path .
```
To install it, then run it as

```
fakeminimap2 the-best-bird.fasta new-reads.fasta 32
```

## Logging
Logging is done using the log crate, and the log level can be set using the RUST_LOG environment variable.

```
RUST_LOG=info cargo run
```

