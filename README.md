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

### Presets
All minimap2 presets should be available:
```rust
let aligner = map_ont();
```
```rust
let aligner = asm20();
```

### Customization
[MapOpts](https://docs.rs/minimap2-sys/0.1.5/minimap2_sys/struct.mm_mapopt_t.html) and [IdxOpts](https://docs.rs/minimap2-sys/0.1.5/minimap2_sys/struct.mm_idxopt_t.html) can be customized with Rust's struct pattern, as well as applying mapping settings.
```rust
Aligner {
    mapopt: MapOpt {
        seed: 42,
        best_n: 1,
        ..Default::default()
    },
    idxopt: IdxOpt {
        k: 21,
        ..Default::default()
    },
    ..map_ont()
}
```
### Working Example
There is a binary called "fakeminimap2" that I am using to test for memory leaks. You can follow the [source code](https://github.com/jguhlin/minimap2-rs/blob/main/fakeminimap2/src/main.rs) for an example. It also shows some helper functions for identifying compression types and FASTA vs FASTQ files. I used my own parsers as they are well fuzzed, but open to removing them or putting them behind a feature wall.

A very simple example would be:
```rust
let mut file = std::fs::File::open(query_file);
let mut reader = BufReader::new(reader);
let mut fasta = Fasta::from_buffer(&mut reader)

for seq in reader {
    let seq = seq.unwrap();
    let alignment = aligner
        .map(&seq.sequence.unwrap(), false, false, None, None)
        .expect("Unable to align");
    println!("{:?}", alignment);
}
```

## Multithreading
Untested, however the thread_local buffer is already set, so theoretically it could work. It's also the weekend, so.... Next week. I may or may not implement it in here, torn between a hold-your-hand library and a lightweight library for those who want to use their own solutions. This may get split into two separate libraries for that very reason (following the [zstd](https://github.com/gyscos/zstd-rs) concept).

# Want feedback
* Many fields are i32 / i8 to mimic the C environment, but would it make more sense to convert to u32 / u8 / usize?
* Let me know pain points

Presets currently look like this:
```rust
Aligner {
    threads: 2,
    ..map_ont()
}
```
or:
```rust
Aligner {
    threads: 2,
    ..preset(Preset::MapOnt)
}
```
or:
```rust
Aligner {
    threads: 2,
    ..Aligner::preset(Preset::MapOnt)
}
```

The second pollutes the namespace less, but the first looks less redundant. Open to opinions.

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