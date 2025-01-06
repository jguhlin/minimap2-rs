### 0.1.23 minimap2 2.28
+ Functions to set flag opts for MapOpt and IdxOpt @dwpeng
+ Fixed memory leak when not dropping mm_idx_t properly. This is done by adding in syntactic sugar in minimap2-sys @jguhlin

### 0.1.22 minimap2 2.28
+ Fixed a memory segfault when re-using a thread local buffer. Not sure why it occurs, but this fix seems to solve it.

### 0.1.21 minimap2 2.28
Contributors to this release: @mbhall88 @rob-p @Sam-Sims @charlesgregory @PB-DB
#### Breaking Changes
+ Map now returns Arc String's to reduce memory allocation for large and/or repetitive jobs
+ map now takes an additional argument, query_name: Option<&[u8]>, possibly solves [#75](https://github.com/jguhlin/minimap2-rs/issues/75) (@rob-p @mbhall88 @jguhlin)
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
+ Experimental Android support (tested on aarch64 and x86_64), solves [#66](https://github.com/jguhlin/minimap2-rs/issues/66)
+ Added flag and option documents
+ Added with_gap_open penalty ergonomic function

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
