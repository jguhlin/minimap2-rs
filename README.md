A rust FFI library for [minimap2](https://github.com/lh3/minimap2/). In development! Feedback appreciated!

[![https://crates.io/crates/minimap2](https://img.shields.io/crates/v/minimap2.svg)](https://crates.io/crates/minimap2)
[![https://docs.rs/minimap2/latest/minimap2/](https://img.shields.io/docsrs/minimap2)](https://docs.rs/minimap2/latest/minimap2/)
[![CircleCI](https://dl.circleci.com/status-badge/img/gh/jguhlin/minimap2-rs/tree/main.svg?style=shield)](https://dl.circleci.com/status-badge/redirect/gh/jguhlin/minimap2-rs/tree/main)
[![codecov](https://codecov.io/gh/jguhlin/minimap2-rs/branch/main/graph/badge.svg?token=huw27ZC6Qy)](https://codecov.io/gh/jguhlin/minimap2-rs)

# Structure
minimap2-sys is the raw FFI bindings to minimap2. minimap2 is the more opinionated, rusty version.

# How to use
## Requirements
```toml
minimap2 = "0.1.21+minimap2.2.28"
```
Also see [Features](#features)

Tested with rustc 1.82.0 and nightly. So probably a good idea to upgrade before running. But let me know if you run into pain points with older versions and will try to fix.

## Minimap2 Version Table
| minimap2-rs | minimap2 |
|-------------|----------|
| 0.1.21      | 2.28     |
| 0.1.20      | 2.28     |
| 0.1.19      | 2.28     |
| 0.1.18      | 2.28     |
| 0.1.17      | 2.27     |
| 0.1.16      | 2.26     |

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
    .map(&seq, false, false, None, None, Some(b"My Sequence Name"))
    .expect("Unable to align");
```

### Presets
All minimap2 presets should be available (see [functions section](https://docs.rs/minimap2/latest/minimap2/)):
```rust
let aligner = map_ont();
let aligner = asm20();
```

**Note** Each preset overwrites different arguments. Using multiple at a time is not technically supported, but will work. Results unknown. So be careful!
It's equivalent to running minimap2 -x map_ont -x short ...

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
        .map(&seq.sequence.unwrap(), false, false, None, None, None)
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
    .with_index_threads(8);
```

### Experimental Rayon support
This _appears_ to work.

```rust
use rayon::prelude::*;

let results = sequences.par_iter().map(|seq| {
    aligner.map(seq.as_bytes(), false, false, None, None, None).unwrap()
}).collect::<Vec<_>>();
```

## Features
The following crate features are available:
* map-file - Enables the ability to map a file directly to a reference. Enabled by deafult
* htslib - Provides an interface to minimap2 that returns rust_htslib::Records
* simde - Enables SIMD Everywhere library in minimap2
* zlib-ng - Enables the use of zlib-ng for faster compression
* curl - Enables curl for htslib
* static - Builds minimap2 as a static library
* sse2only - Builds minimap2 with only SSE2 support

Map-file is a *default* feature and enabled unless otherwise specified.

## Missing Features
* setting mismatch penalty for base transitions [minimap 2.27 release notes](https://github.com/lh3/minimap2/releases/tag/v2.27)
* Generate ds tags to indicate uncertainty in indels

Potentially others. Please create an issue! 

## Building for MUSL
Follow these [instructions](https://github.com/rust-cross/rust-musl-cross#prebuilt-images).

In brief, using bash shell:
```bash
docker pull messense/rust-musl-cross:x86_64-musl
alias rust-musl-builder='docker run --rm -it -v "$(pwd)":/home/rust/src messense/rust-musl-cross:x86_64-musl'
rust-musl-builder cargo build --release
```

Minimap2 is tested on x86_64 and aarch64 (arm64). Other platforms may work, please open an issue if minimap2 compiles but minimap2-rs does not.

### Features tested with MUSL
* `htslib` - **Success**
* `simde` - **Success**

# Tools using this binding
- [Chopper](https://github.com/wdecoster/chopper) - Long read trimming and filtering
- [mappy-rs](https://github.com/Adoni5/mappy-rs) - Drop-in multi-threaded replacement for python's mappy
- [HiFiHLA](https://github.com/PacificBiosciences/hifihla) - HLA star-calling tool for PacBio HiFi data
- [STRdust](https://github.com/wdecoster/STRdust) - Tandem repeat genotyper for long reads
- [oarfish](https://github.com/COMBINE-lab/oarfish) - transcript quantification from long-read RNA-seq data

# Next things todo
* Multi-thread guide (tokio async threads or use crossbeam queue and traditional threads?)
* Iterator interface for map_file
* -sys Possible to decouple from pthread?

# Citation
Please cite the appropriate minimap2 papers if you use this in your work, as well as this library.

## DOI for this library

## Minimap2 Papers

> Li, H. (2018). Minimap2: pairwise alignment for nucleotide sequences.
> *Bioinformatics*, **34**:3094-3100. [doi:10.1093/bioinformatics/bty191][doi]

and/or:

> Li, H. (2021). New strategies to improve minimap2 alignment accuracy.
> *Bioinformatics*, **37**:4572-4574. [doi:10.1093/bioinformatics/btab705][doi2]

# Changelog
### 0.1.21 minimap2 2.28
Contributors to this release: @mbhall88 @rob-p @Sam-Sims @charlesgregory @PB-DB
#### Breaking Changes
+ Map now returns Arc String's to reduce memory allocation for large and/or repetitive jobs
+ map now takes an additional argument, query_name: Option<impl AsRef<[u8]>>, possibly solves [#75](https://github.com/jguhlin/minimap2-rs/issues/75) (@rob-p @mbhall88 @jguhlin)
+ Arc the Index, to prevent double-frees, solves [#71](https://github.com/jguhlin/minimap2-rs/issues/71)
+ Map file now passes in query name, which should help with [#75](https://github.com/jguhlin/minimap2-rs/issues/75)
+ Supplementary flag now better detected (@rob-p)
+ FIX: Cigar string missing softclip operation (@Sam-Sims)

#### Migration Guide
+ Make all setting changes before calling a with_index, with_seq's function
+ Change all map calls to include a query name, or None if not needed

### Other Changes
+ Add ergonomic functions n_seq and get_seq.
+ Better docs on applying presets, solves [#84](https://github.com/jguhlin/minimap2-rs/issues/84)
+ Better detection of target arch c_char's and ptr's, solves [#82](https://github.com/jguhlin/minimap2-rs/issues/82)
+ Support for M1 Mac compilation and addition of github workflows to test it, solving [#81](https://github.com/jguhlin/minimap2-rs/issues/81)
+ Rayon test, so some support, closes [#5](https://github.com/jguhlin/minimap2-rs/issues/5)
+ Static str's and now static CStr's
+ FIX: memory leak due to sequences allocated by minimap2 not being freed @charlesgregory
+ Add Send + Sync to Aligner, along with unit test @PB-DB
+ Only use rust-htslib/curl when curl feature is enabled @PB-DB
+ Mark bam::Record objects as supplementary @PB-DB
+ Experimental Android support (tested on aarch64 and x86_64), solves [#66](https://github.com/jguhlin/minimap2-rs/issues/66)

### 0.1.20 minimap2 2.28
+ Fix htslib errors. No update to -sys crate needed.

### 0.1.19 minimap2 2.28
+ Fix memory leak by @charlesgregory

### 0.1.18 minimap2 2.28
+ Update to minimap2 v2.28 @jguhlin
+ Support for lrhqae preset @jguhlin

### 0.1.17 minimap2 2.27
* Mark bam::Record objects as supplementary. #52  @PB-DB
* Only use rust-htslib/curl when curl feature is enabled. #53 @PB-DB
* Update to minimap2 v2.27 @jguhlin
* Switch to needletail for reading fast files (features map-file) @jguhlin
* Convert functions to take slices of vectors instead of refs to vecs `&[Vec<u8>]` instead of `&Vec<Vec<u8>>` @jguhlin
* _breaking_ Curl is no longer a default option for htslib, please re-enable it as needed with cargo.toml features
* _breaking_ Now using needletail for map-files, enabled by default. However, compression algorithms are disabled. Please enable with cargo.toml features
* Experimental rayon support
* aligner.with_cigar_clipping() to add soft clipping to the CIGAR vec (with_cigar() still adds to only the string, following the minimap2 outputs for PAF)
* _breaking_ .with_threads(_) is now .with_index_threads(_) to make it more clear

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
