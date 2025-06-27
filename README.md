[![https://crates.io/crates/minimap2](https://img.shields.io/crates/v/minimap2.svg)](https://crates.io/crates/minimap2)
[![https://docs.rs/minimap2/latest/minimap2/](https://img.shields.io/docsrs/minimap2)](https://docs.rs/minimap2/latest/minimap2/)
[![CircleCI](https://dl.circleci.com/status-badge/img/gh/jguhlin/minimap2-rs/tree/main.svg?style=shield)](https://dl.circleci.com/status-badge/redirect/gh/jguhlin/minimap2-rs/tree/main)
[![codecov](https://codecov.io/gh/jguhlin/minimap2-rs/branch/main/graph/badge.svg?token=huw27ZC6Qy)](https://codecov.io/gh/jguhlin/minimap2-rs)

# Introduction
A rust FFI wrapper library for [minimap2](https://github.com/lh3/minimap2/) with support for SIMDe, htslib, zlib-ng, and curl.

# How to use
## Requirements
```toml
minimap2 = "0.1.26+minimap2.2.30"
```
Also see [Features](#features)

Tested with rustc 1.86.0. Recommended to use the latest stable version of rust, but if you need support for an older version, please open an issue.

## Minimap2 Version Table
| minimap2-rs     | minimap2 |
|-----------------|----------|
| 0.1.25 - 0.1.26 | 2.30     |
| 0.1.24          | 2.29     |
| 0.1.18 - 0.1.23 | 2.28     |
| 0.1.17          | 2.27     |
| 0.1.16          | 2.26     |

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
let mut aligner: Aligner<PresetSet> = Aligner::builder().map_ont();
aligner.mapopt.seed = 42;
aligner.mapopt.best_n = 1;
aligner.idxopt.k = 21;
self.mapopt.flag |= MM_F_COPY_COMMENT as i64; // Setting a flag. If you do this frequently, open an [issue](https://github.com/jguhlin/minimap2-rs/issues/new) asking for an ergonomic function!
self.idxopt.flag |= MM_I_HPC as i32;
```

See [full list of options](#minimap2-mapping-and-indexing-options) below.

### Working Example

#### Examples Directory
There are two working examples directly in this repo. In both instances below, 64 is the number of threads to allocate.

**Channel-based multi-threading**
```
cargo run --example channels -- reference_genome.fasta reads.fasta 64
```

**Rayon-based multi-threading**
```
cargo run --example rayon -- reference_genome.fasta reads.fasta 64
```

**Depending on your needs** you can probably do just fine with Rayon. But for very large implementations, interactivity, or limited memory, using channels may be the way to go.

#### Fakeminimap2

There is a binary called "fakeminimap2" which demonstrates basic usage and multithreading using channels or rayon. You can find it [in this repo](https://github.com/jguhlin/minimap2-rs/tree/main/fakeminimap2) for an example. It it much more fully featured example, with an output interface, some mouse support, and interaction.

#### Code Examples

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

Adjust the number of threads used to build the index:
```rust
let mut aligner = Aligner::builder()
    .map_ont()
    .with_index_threads(8);
```

### Experimental Rayon support
This _appears_ to work. See [fakeminimap2](https://github.com/jguhlin/minimap2-rs/tree/main/fakeminimap2) for full implementation.

```rust
use rayon::prelude::*;

let results = sequences.par_iter().map(|seq| {
    aligner.map(seq.as_bytes(), false, false, None, None, None).unwrap()
}).collect::<Vec<_>>();
```

### Arc cloning the Aligner
Also works. Otherwise directly cloning the aligner will Arc clone the internal index.

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
Create an [issue](https://github.com/jguhlin/minimap2-rs/issues/new) if you need any of the following:
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
- [lrge](https://github.com/mbhall88/lrge) - Long Read-based Genome size Estimation from overlaps
- [mmr](https://github.com/arcInstitute/mmr) - A minimap2-based aligner with BINSEQ file format support

# Notes
## Memory management
Minimap2 sets cap_kalloc to ~500Mb, so for 32 threads this can be ~16Gb. You can set this manually with `aligner.mapopt.cap_kalloc`. This is a good idea if you are using a lot of threads, or have very long running jobs.

```
    let per_thread_cap_kalloc = (1_000_000_000_f64 / (args.threads as f64)).ceil() as i64;
    aligner.mapopt.cap_kalloc = per_thread_cap_kalloc;
```

## best_n
* Minimap2 outputs and results are sensitive to the best_n parameter. Set it manually or be prepared for it to be changed upstream (potentially!)

# Next things todo
* Iterator interface for map_file
* -sys Possible to decouple from pthread?

# Citation
Please cite the appropriate minimap2 papers if you use this in your work, as well as this library.

## DOI for this library
... coming soon ...

## Minimap2 Papers

> Li, H. (2018). Minimap2: pairwise alignment for nucleotide sequences.
> *Bioinformatics*, **34**:3094-3100. [doi:10.1093/bioinformatics/bty191][doi]

and/or:

> Li, H. (2021). New strategies to improve minimap2 alignment accuracy.
> *Bioinformatics*, **37**:4572-4574. [doi:10.1093/bioinformatics/btab705][doi2]

# Minimap2 Mapping and Indexing Options

See [customization](#customization) for how to use these.

## Mapping Options (`MapOpt` in rust, alias for `mm_mapopt_t`)

| Field Name        | Type                    | Description                                                          |
|-------------------|-------------------------|----------------------------------------------------------------------|
| `flag`            | `i64`                   | Flags to control mapping behavior (bitwise flags).                   |
| `seed`            | `c_int`                 | Random seed for mapping.                                             |
| `sdust_thres`     | `c_int`                 | Threshold for masking low-complexity regions using SDUST.            |
| `max_qlen`        | `c_int`                 | Maximum query length.                                                |
| `bw`              | `c_int`                 | Bandwidth for alignment of short reads.                              |
| `bw_long`         | `c_int`                 | Bandwidth for alignment of long reads.                               |
| `max_gap`         | `c_int`                 | Maximum gap allowed in mapping.                                      |
| `max_gap_ref`     | `c_int`                 | Maximum gap allowed on the reference.                                |
| `max_frag_len`    | `c_int`                 | Maximum fragment length for paired-end reads.                        |
| `max_chain_skip`  | `c_int`                 | Maximum number of seeds to skip in chaining.                         |
| `max_chain_iter`  | `c_int`                 | Maximum number of chaining iterations.                               |
| `min_cnt`         | `c_int`                 | Minimum number of seeds required for a chain.                        |
| `min_chain_score` | `c_int`                 | Minimum score for a chain to be considered.                          |
| `chain_gap_scale` | `f32`                   | Scaling factor for chain gap penalty.                                |
| `chain_skip_scale`| `f32`                   | Scaling factor for chain skipping.                                   |
| `rmq_size_cap`    | `c_int`                 | Size cap for RMQ (Range Minimum Query).                              |
| `rmq_inner_dist`  | `c_int`                 | Inner distance for RMQ rescue.                                       |
| `rmq_rescue_size` | `c_int`                 | Size threshold for RMQ rescue.                                       |
| `rmq_rescue_ratio`| `f32`                   | Rescue ratio for RMQ.                                                |
| `mask_level`      | `f32`                   | Level at which to mask repetitive seeds.                             |
| `mask_len`        | `c_int`                 | Length of sequences to mask.                                         |
| `pri_ratio`       | `f32`                   | Ratio threshold for primary alignment selection.                     |
| `best_n`          | `c_int`                 | Maximum number of best alignments to retain.                         |
| `alt_drop`        | `f32`                   | Score drop ratio for alternative mappings.                           |
| `a`               | `c_int`                 | Match score.                                                         |
| `b`               | `c_int`                 | Mismatch penalty.                                                    |
| `q`               | `c_int`                 | Gap open penalty.                                                    |
| `e`               | `c_int`                 | Gap extension penalty.                                               |
| `q2`              | `c_int`                 | Gap open penalty for long gaps.                                      |
| `e2`              | `c_int`                 | Gap extension penalty for long gaps.                                 |
| `transition`      | `c_int`                 | Penalty for transitions in spliced alignment.                        |
| `sc_ambi`         | `c_int`                 | Score for ambiguous bases.                                           |
| `noncan`          | `c_int`                 | Allow non-canonical splicing (boolean flag).                         |
| `junc_bonus`      | `c_int`                 | Bonus score for junctions.                                           |
| `zdrop`           | `c_int`                 | Z-drop score for alignment extension stopping.                       |
| `zdrop_inv`       | `c_int`                 | Inverse Z-drop score.                                                |
| `end_bonus`       | `c_int`                 | Bonus score for aligning to the ends of sequences.                   |
| `min_dp_max`      | `c_int`                 | Minimum score to consider a DP alignment valid.                      |
| `min_ksw_len`     | `c_int`                 | Minimum length for performing Smith-Waterman alignment.              |
| `anchor_ext_len`  | `c_int`                 | Length for anchor extension.                                         |
| `anchor_ext_shift`| `c_int`                 | Shift for anchor extension.                                          |
| `max_clip_ratio`  | `f32`                   | Maximum allowed clipping ratio.                                      |
| `rank_min_len`    | `c_int`                 | Minimum length for rank filtering.                                   |
| `rank_frac`       | `f32`                   | Fraction for rank filtering.                                         |
| `pe_ori`          | `c_int`                 | Expected orientation of paired-end reads.                            |
| `pe_bonus`        | `c_int`                 | Bonus score for proper paired-end alignment.                         |
| `mid_occ_frac`    | `f32`                   | Fraction for mid-occurrence filtering.                               |
| `q_occ_frac`      | `f32`                   | Fraction for query occurrence filtering.                             |
| `min_mid_occ`     | `i32`                   | Minimum mid-occurrence threshold.                                    |
| `max_mid_occ`     | `i32`                   | Maximum mid-occurrence threshold.                                    |
| `mid_occ`         | `i32`                   | Mid-occurrence cutoff value.                                         |
| `max_occ`         | `i32`                   | Maximum occurrence cutoff value.                                     |
| `max_max_occ`     | `i32`                   | Maximum allowed occurrence value.                                    |
| `occ_dist`        | `i32`                   | Distribution of occurrences for filtering.                           |
| `mini_batch_size` | `i64`                   | Size of mini-batches for processing.                                 |
| `max_sw_mat`      | `i64`                   | Maximum size of Smith-Waterman matrices.                             |
| `cap_kalloc`      | `i64`                   | Memory allocation cap for kalloc.                                    |
| `split_prefix`    | `*const c_char`         | Prefix for splitting output files.                                   |

## Mapping Flags (`MM_F_*`)

These can be set with helper functions. Please see the docs for IdxOpt and MapOpt.

| Flag Constant           | Value          | Description                                                     |
|-------------------------|----------------|-----------------------------------------------------------------|
| `MM_F_NO_DIAG`          | `1`            | Skip seed pairs on the same diagonal.                           |
| `MM_F_NO_DUAL`          | `2`            | Do not compute reverse complement of seeds.                     |
| `MM_F_CIGAR`            | `4`            | Compute CIGAR string.                                           |
| `MM_F_OUT_SAM`          | `8`            | Output alignments in SAM format.                                |
| `MM_F_NO_QUAL`          | `16`           | Do not output base quality in SAM.                              |
| `MM_F_OUT_CG`           | `32`           | Output CIGAR in CG format (Compact CIGAR).                      |
| `MM_F_OUT_CS`           | `64`           | Output cs tag (difference string) in SAM/PAF.                   |
| `MM_F_SPLICE`           | `128`          | Enable spliced alignment (for RNA-seq).                         |
| `MM_F_SPLICE_FOR`       | `256`          | Only consider the forward strand for spliced alignment.         |
| `MM_F_SPLICE_REV`       | `512`          | Only consider the reverse strand for spliced alignment.         |
| `MM_F_NO_LJOIN`         | `1024`         | Disable long join for gapped alignment.                         |
| `MM_F_OUT_CS_LONG`      | `2048`         | Output cs tag in long format.                                   |
| `MM_F_SR`               | `4096`         | Perform split read alignment (for short reads).                 |
| `MM_F_FRAG_MODE`        | `8192`         | Fragment mode for paired-end reads.                             |
| `MM_F_NO_PRINT_2ND`     | `16384`        | Do not output secondary alignments.                             |
| `MM_F_2_IO_THREADS`     | `32768`        | Use two I/O threads during mapping.                             |
| `MM_F_LONG_CIGAR`       | `65536`        | Use long CIGAR (>65535 operations).                             |
| `MM_F_INDEPEND_SEG`     | `131072`       | Map segments independently in multiple mapping.                 |
| `MM_F_SPLICE_FLANK`     | `262144`       | Add flanking bases for spliced alignment.                       |
| `MM_F_SOFTCLIP`         | `524288`       | Perform soft clipping at ends.                                  |
| `MM_F_FOR_ONLY`         | `1048576`      | Only map the forward strand of the query.                       |
| `MM_F_REV_ONLY`         | `2097152`      | Only map the reverse complement of the query.                   |
| `MM_F_HEAP_SORT`        | `4194304`      | Use heap sort for mapping.                                      |
| `MM_F_ALL_CHAINS`       | `8388608`      | Output all chains (may include suboptimal chains).              |
| `MM_F_OUT_MD`           | `16777216`     | Output MD tag in SAM.                                           |
| `MM_F_COPY_COMMENT`     | `33554432`     | Copy comment from FASTA/Q to SAM output.                        |
| `MM_F_EQX`              | `67108864`     | Use =/X instead of M in CIGAR.                                  |
| `MM_F_PAF_NO_HIT`       | `134217728`    | Output unmapped reads in PAF format.                            |
| `MM_F_NO_END_FLT`       | `268435456`    | Disable end flanking region filtering.                          |
| `MM_F_HARD_MLEVEL`      | `536870912`    | Hard mask low-complexity regions.                               |
| `MM_F_SAM_HIT_ONLY`     | `1073741824`   | Output only alignments in SAM (no headers).                     |
| `MM_F_RMQ`              | `2147483648`   | Use RMQ for read mapping quality estimation.                    |
| `MM_F_QSTRAND`          | `4294967296`   | Consider query strand in mapping.                               |
| `MM_F_NO_INV`           | `8589934592`   | Disable inversion in alignment.                                 |
| `MM_F_NO_HASH_NAME`     | `17179869184`  | Do not hash read names (for reproducibility).                   |
| `MM_F_SPLICE_OLD`       | `34359738368`  | Use old splice alignment model.                                 |
| `MM_F_SECONDARY_SEQ`    | `68719476736`  | Output sequence of secondary alignments.                        |
| `MM_F_OUT_DS`           | `137438953472` | Output detailed alignment score (ds tag).                       |

## Index Options (`IdxOpt` in rust, alias for `mm_idxopt_t`)

| Field Name        | Type           | Description                                                     |
|-------------------|----------------|-----------------------------------------------------------------|
| `k`               | `c_short`      | K-mer size (mer length).                                        |
| `w`               | `c_short`      | Minimizer window size.                                          |
| `flag`            | `c_short`      | Flags to control indexing behavior (bitwise flags).             |
| `bucket_bits`     | `c_short`      | Number of bits for the size of hash table buckets.              |
| `mini_batch_size` | `i64`          | Size of mini-batches for indexing (number of bases).            |
| `batch_size`      | `u64`          | Total batch size for indexing (number of bases).                |

## Indexing Flags (`MM_I_*`)

These can be set with helper functions. Please see the docs for IdxOpt and MapOpt.

| Flag Constant     | Value  | Description                                              |
|-------------------|--------|----------------------------------------------------------|
| `MM_I_HPC`        | `1`    | Use homopolymer-compressed k-mers for indexing.          |
| `MM_I_NO_SEQ`     | `2`    | Do not store sequences in the index.                     |
| `MM_I_NO_NAME`    | `4`    | Do not store sequence names in the index.                |

# Changelog
See [CHANGELOG.md](CHANGELOG.md)

# Funding
![Genomics Aotearoa](info/genomics-aotearoa.png)
