A rust FFI library for [minimap2](https://github.com/lh3/minimap2/). In development! Feedback appreciated!

[![https://crates.io/crates/minimap2](https://img.shields.io/crates/v/minimap2.svg)](https://crates.io/crates/minimap2)
[![https://docs.rs/minimap2/latest/minimap2/](https://img.shields.io/docsrs/minimap2)](https://docs.rs/minimap2/latest/minimap2/)
[![CircleCI](https://dl.circleci.com/status-badge/img/gh/jguhlin/minimap2-rs/tree/main.svg?style=shield)](https://dl.circleci.com/status-badge/redirect/gh/jguhlin/minimap2-rs/tree/main)
[![codecov](https://codecov.io/gh/jguhlin/minimap2-rs/branch/main/graph/badge.svg?token=huw27ZC6Qy)](https://codecov.io/gh/jguhlin/minimap2-rs)


# Structure
minimap2-sys is the library of the raw FFI bindings to minimap2. minimap2 is the more rusty version.

# How to use
## Requirements
```toml
minimap2 = "0.1.14+minimap2.2.26"
```
Also see [Features](#features)

Tested with rustc 1.64.0 and nightly. So probably a good idea to upgrade before running. But let me know if you run into pain points with older versions and will try to fix!
```bash
rustup update
```

## Usage
Create an Aligner 

```rust
let mut aligner = Aligner::builder()
    .map_ont()
    .with_threads(8)
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
Multithreading is supported, for implementation example see [fakeminimap2](https://github.com/jguhlin/minimap2-rs/blob/main/fakeminimap2/src/main.rs). Minimap2 also supports threading itself, and will use a minimum of 3 cores for building the index. Multithreading for mapping is left to the end-user.

```rust
let mut aligner = Aligner::builder()
    .map_ont()
    .with_threads(8);
```

## Features
The following crate features are available:
* `mm2-fast` - Replace minimap2 with [mm2-fast](https://github.com/bwa-mem2/mm2-fast). This is likely not portable.
* `htslib` - Support output of bam/sam files using htslib.
* `simde` - Compile minimap2 / mm2-fast with [simd-everywhere](https://github.com/simd-everywhere/simde) support. 
* `map-file` - *Default* - Convenience function for mapping an entire file. Caution, this is single-threaded. 
* `sse2only` - Compiles for SSE2 support only (Default is to try to compile for SSE4.1, SSE2 only is default on aarch64)

Map-file is a *default* feature and enabled unless otherwise specified.

## Building for MUSL
Follow these [instructions](https://github.com/rust-cross/rust-musl-cross#prebuilt-images).

In brief, using bash shell:
```bash
docker pull messense/rust-musl-cross:x86_64-musl
alias rust-musl-builder='docker run --rm -it -v "$(pwd)":/home/rust/src messense/rust-musl-cross:x86_64-musl'
rust-musl-builder cargo build --release
```

Please note minimap2 is only tested for x86_64. Other platforms may work, please open an issue if minimap2 compiles but minimap2-rs does not.

### Features tested with MUSL
* `mm2-fast` - **Fail**
* `htslib` - **Success**
* `simde` - **Success**

# Tools using this binding
- [Chopper](https://github.com/wdecoster/chopper) - Long read trimming and filtering
- [mappy-rs](https://github.com/Adoni5/mappy-rs) - Drop-in multi-threaded replacement for python's mappy
- [HiFiHLA](https://github.com/PacificBiosciences/hifihla) - HLA star-calling tool for PacBio HiFi data

# Want feedback
* Many fields are i32 / i8 to mimic the C environment, but would it make more sense to convert to u32 / u8 / usize?
* Let me know pain points!

# Next things todo
* Print other tags so we can have an entire PAF format
* -sys Compile with SSE2 / SSE4.1 (auto-detect, but also make with features)
* Multi-thread guide (tokio async threads or use crossbeam queue and traditional threads?)
* Iterator interface for map_file
* MORE TESTS
* -sys Get SSE working with "sse" feature (compiles and tests work in -sys crate, but not main crate)
* -sys Possible to decouple from pthread?
* -sys Enable Lisa-hash for mm2-fast? But must handle build arguments from the command-line.

# Citation
You should cite the minimap2 papers if you use this in your work.

> Li, H. (2018). Minimap2: pairwise alignment for nucleotide sequences.
> *Bioinformatics*, **34**:3094-3100. [doi:10.1093/bioinformatics/bty191][doi]

and/or:

> Li, H. (2021). New strategies to improve minimap2 alignment accuracy.
> *Bioinformatics*, **37**:4572-4574. [doi:10.1093/bioinformatics/btab705][doi2]

# Changelog
### 0.1.17 minimap2 2.27
* Mark bam::Record objects as supplementary. #52  @PB-DB
* Only use rust-htslib/curl when curl feature is enabled. #53 @PB-DB
* Update to minimap2 v2.27 @jguhlin

### 0.1.16 minimap2 2.26
* Much better cross compilation support thanks to @Adoni5

### 0.1.15 minimap2 2.26 
* Compilation on aarch64 thanks to @leiste375
* README corrections thanks to @wdecoster
* Better support for static builds / linking
* Update fffx to a version that uses bytelines without tokio. Drastically reduces compile times and dependency tree.

### 0.1.14 minimap2 2.26
* Memory leak fixed by @Adoni5
* Updated deps

### 0.1.13 minimap2 2.26
* Add with_seq to support indexing a single sequence (as per mappy: https://github.com/lh3/minimap2/blob/master/python/mappy.pyx#L115)
* minimap2-rs: update rust-htslib deps
* simdutf8 now optional dependency requiring map-file feature to be enabled
* Support soft-clipping string in CIGAR. WARNING: Does not support hard clipping. Please open an issue if you need this.
* Update minimap to 2.26
* Not convinced SSE41/SSE2 are working properly. Recommend simde.

### 0.1.11 
* HTS lib: add support for optional quality scores by @eharr

### 0.1.10
* HTS lib support by @eharr
* HTS lib: Output sam/bam files by @eharr
* More tests by @eharr
* Display impl for Strand thanks to @ahcm
* Update minimap2-sys to latest version by @jguhlin
* -sys crate mm2fast added as additional backend by @jguhlin
* zlib dep changes by @jguhlin (hopefully now it is more portable and robust)
* -sys crate now supports SIMDe

## 0.1.9
* Thanks for @Adoni5 for switching to builder pattern, and @eharr for adding additional fields to alignment.
* Do not require libclang for normal compilation.
## 0.1.8
* Multithreading support (use less raw pointers, and treat more like rust Struct's)
## 0.1.7
* use libc instead of std:ffi::c_int as well
## 0.1.6 
* Support slightly older versions of rustc by using libc:: rather than std::ffi for c_char (Thanks dwpeng!)
* Use fffx module for fasta/q parsing

# Funding
![Genomics Aotearoa](info/genomics-aotearoa.png)
