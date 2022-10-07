In development!

# How to use
## Requirements
Clang is required to build (probably....)
```toml
minimap2 = "1.1.5"
```

## Use
Create an Aligner 

```rust
let mut aligner = Aligner {
    threads: 8,
    ..map_ont()
}
.with_cigar()
.with_index("ReferenceFile.fasta", None)
.expect("Unable to build index");
```

Align a sequence:
```rust
let seq: Vec<u8> = b"ACTGACTCACATCGACTACGACTACTAGACACTAGACTATCGACTACTGACATCGA";
let alignment = aligner
    .map(seq, false, false, None, None)
    .expect("Unable to align");
```

# Want feedback
* Many fields are i32 / i8 to mimic the C environment, but would it make more sense to convert to u32 / u8 / usize?
* Let me know pain points

# Pain Points
Probably not freeing C memory somewhere.... Not sure yet, if so it's just leaking a little...

# Next things todo
* Print other tags so we can have an entire PAF format
* Compile with SSE2 / SSE4.1 / SIMDe (auto-detect, but also make with features)
* Multi-thread guide (tokio async threads or use crossbeam queue and traditional threads?)
* Maybe should be split into 2 more libraries, minimap2-safe (lower but safe level) and minimap2 (high-level api) like zstd? - Then people can implement threading as they like or just fall back to a known-decent implementation?
* Iterator interface for map_file


# Changelog
¯\\_(ツ)_/¯