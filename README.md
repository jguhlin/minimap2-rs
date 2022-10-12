A rust FFI library for [minimap2](https://github.com/lh3/minimap2/). In development! Feedback appreciated!

![https://crates.io/crates/minimap2](https://img.shields.io/crates/v/minimap2.svg)
![https://docs.rs/minimap2/latest/minimap2/](https://img.shields.io/docsrs/minimap2)

# Structure
minimap2-sys is the library of the raw FFI bindings to minimap2. minimap2 is the most rusty version.

# How to use
## Requirements
Clang is required to build (probably....)
```toml
minimap2 = "1.1.6"
```

Tested with rustc 1.64.0 and nightly. So probably a good idea to upgrade before running. But let me know if you run into pain points with older versions and will try to fix!
```bash
rustup update
```

## Usage
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
    .map(&seq, false, false, None, None)
    .expect("Unable to align");
```

### Presets
All minimap2 presets should be available (see [functions section](https://docs.rs/minimap2/latest/minimap2/)):
```rust
let aligner = map_ont();
let aligner = asm20();
```

### Customization
[MapOpts](https://docs.rs/minimap2-sys/0.1.5/minimap2_sys/struct.mm_mapopt_t.html) and [IdxOpts](https://docs.rs/minimap2-sys/0.1.5/minimap2_sys/struct.mm_idxopt_t.html) can be customized with Rust's struct pattern, as well as applying mapping settings. Inspired by [bevy](https://bevyengine.org/).
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

Alignment functions return a [Mapping](https://docs.rs/minimap2/latest/minimap2/struct.Mapping.html) struct. The [Alignment](https://docs.rs/minimap2/latest/minimap2/struct.Alignment.html) struct is only returned when the [Aligner](https://docs.rs/minimap2/latest/minimap2/struct.Aligner.html) is created using [.with_cigar()](https://docs.rs/minimap2/latest/minimap2/struct.Aligner.html#method.with_cigar).

A very simple example would be:
```rust
let mut file = std::fs::File::open(query_file);
let mut reader = BufReader::new(reader);
let mut fasta = Fasta::from_buffer(&mut reader)

for seq in reader {
    let seq = seq.unwrap();
    let alignment: Vec<Mapping> = aligner
        .map(&seq.sequence.unwrap(), false, false, None, None)
        .expect("Unable to align");
    println!("{:?}", alignment);
}
```

There is a map_file function that works on an entire file, but it is not-lazy and thus not suitable for large files. It may be removed in the future or moved to a separate lib.

```rust
let mappings: Result<Vec<Mapping>> = aligner.map_file("query.fa", false, false);
```

## Multithreading
Untested, however the thread_local buffer is already set, so theoretically it could work. I may or may not implement it in here, torn between a hold-your-hand library and a lightweight library for those who want to use their own solutions. This may get split into two separate libraries for that very reason (following the [zstd](https://github.com/gyscos/zstd-rs) concept).

So far multithreading only works for building the index and not for mapping.

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
* MORE TESTS

# Citation
You should cite the minimap2 papers if you use this in your work. If you use this extensively, let me know and I'll add a way to cite this project as well with version (Zenodo, probably).

> Li, H. (2018). Minimap2: pairwise alignment for nucleotide sequences.
> *Bioinformatics*, **34**:3094-3100. [doi:10.1093/bioinformatics/bty191][doi]

and/or:

> Li, H. (2021). New strategies to improve minimap2 alignment accuracy.
> *Bioinformatics*, **37**:4572-4574. [doi:10.1093/bioinformatics/btab705][doi2]


# Changelog
## 1.1.6 
* Support slightly older versions of rustc by using libc:: rather than std::ffi for c_char (Thanks dwpeng!)
* Use fffx module for fasta/q parsing


![Genomics Aotearoa](info/genomics-aotearoa.png)