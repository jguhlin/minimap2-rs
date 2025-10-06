---
title: 'Minimap2-rs: Rust bindings for Minimap2'
tags:
    - Rust
    - sequencing
    - genomics
    - sequence alignment
    - language binding
    - long reads
authors:
    - name: Joseph Guhlin
      orcid: 0000-0003-3264-7178
      affiliation: "1, 2"
affiliations:
    - name: Genomics Aotearoa, University of Otago, Dunedin, New Zealand
      index: 1
    - name: Department of Biochemistry, University of Otago, Dunedin, New Zealand
      index: 2
date: 7 October 2025
bibliography: paper.bib

---

# Summary

Long-read sequence alignment underpins modern genomic research, enabling highly contiguous de novo assemblies, structural variant detection, RNA isoform exploration, and many other applications. Minimap2, the long read sequence aligner, stands at the center of these developments, offering fast alignment for long and noisy reads. In parallel, the Rust programming language has seen rapid adoption in the scientific community, providing memory safety, concurrency, and high performance. We present Minimap2-rs, a suite of Rust crates that interface with Minimap2, lowering the barrier to using this versatile aligner in Rust-based applications. 
  
The top-level minimap2 crate offers an opinionated, flexible idiomatic API that manages memory and simplifies foreign function interface (FFI) complexities. A companion Python library, minimappers2, returns high-performance dataframes via Polars, while allowing for intuitive multithreading and data handling. Meanwhile, the minimap2-sys crate provides direct, low-level bindings to Minimap2 for specialized use cases. This modular design facilitates both rapid prototyping and production-ready pipelines, illustrated by two example implementations of multithreading (multi-producer single-consumer channels and Rayon). Continuous integration across x86_64 and arm64 architectures ensures reliability on Linux, macOS, and Android, with tested support for musl-based portability. Already adopted by multiple tools for genome size estimation, tandem repeat genotyping, and transcriptome analysis, Minimap2-rs offers a stable, open-source foundation for building Rust-powered bioinformatics solutions while preserving Minimap2’s speed and flexibility.

# Statement of Need

Long-read sequence alignment has changed the foundation of recent genomic advances. Thanks to long-read sequence alignment, advances such as the telomere-to-telomere Human genome assembly [@nurk2022complete] were completed, the cost to assemble a _de novo_ genome has significantly decreased and contiguity has increased [@van2023nanopore; @koren2015one; @murigneux2020comparison; @hifiasm; @flye], and is used for RNA-based applications [@jain2022advances]. The primary algorithm supporting nearly all of these advancements is found in the tool Minimap2, which can align long-read sequences to large references, accounting for the high error rate that may be present in long reads [@minimap2; @minimap2new]. Minimap2 can also accommodate spliced alignments, short-read alignments, and assembly-to-assembly alignments.

Meanwhile, the Rust programming language has seen significant adoption, including amongst scientists [@rust; @bugden2022rust; @rustscience]. Rust brings memory safety with a strong ownership model and performance to applications and libraries, and consistent build tooling [@bugden2022rust]. This ultimately allows for highly portable applications with strong concurrency idioms, encouraging high-performance applications by default. With the growing adoption of Rust, there are now several tools and libraries in Rust for bioinformatics [@huey2024bigtools; @buffalo2024granges; @rustbio; @chan2024next].

Using Minimap2 directly in Rust requires complicated foreign function interface (FFI) calls. Here, I present Minimap2-rs, which provides FFI bindings to the Minimap2 library, exposing the underlying API directly to Rust and providing a more idiomatic Rust API to work with Minimap2, easing the barrier to entry for others to interface with Minimap2. Minimap2-rs is open source and publicly available through crates.io, the standard library registry for Rust, and GitHub. Minimap2-rs is already used by multiple tools for sequence cleaning [@de2023nanopack2], genotyping tandem repeats [@strdust], long-read transcriptome quantification [@oarfish], and long-read-based genome size estimation [@hall_genome_2024]. Minimap2-rs compiles on the x86_64 and arm64 architectures for Linux, Mac, and Android.

# Implementation

Minimap2 is a command-line tool and library, and an official Python library, Mappy, is available separately. Minimap2 uses single-instruction multiple-data (SIMD) CPU features to achieve better speed and parallelization, allowing it to work with large datasets and long-reads. Minimap2-rs consists of three libraries (called 'crates' in Rust): Minimap2, minimappers2, and minimap2-sys. The primary point of entry, the Minimap2 Rust crate (further referred to as minimap2-rs) provides an opinionated, memory-safe library for working with Minimap2 functions via the foreign function interface (FFI) and the given output in Rust data structures (structs), converting base types between C and Rust automatically. Minimappers2 is a Python library wrapping the Minimap2 Rust library, providing seamless multi-threading and returning results via the high-performance, memory-efficient Polars data frames library. Minimap2-sys wraps the Minimap2 library and allows direct interface with the C functions with Rust via unsafe code [@rust]. A functional fourth tool is available, fakeminimap2, which serves as a functioning exemplar of two common multithreading approaches.

Minimap2-rs is the primary interface for using Minimap2 in downstream Rust applications. An aligner struct is created, and presets, mapping, and indexing options are configured through this struct. All presets from the Minimap2 command-line software (e.g., map-ont, map-pb, map-hifi, asm20) and low-level access to further index and mapping options in the Minimap2 C library are available. The crate is configurable with optional features (\ref{crate-features}). Minimap2 is built using Rust's tooling, thus adding this library only requires adding it as a dependency. The code repository provides two examples of multithreading implementation with minimap2-rs using multi-producer single-consumer channels (MPSC) and parallelization using the popular Rayon crate.

: Rust crate-level features available for `minimap2-rs`. []{\label{crate-features}}
Rust features enable conditional compilation and reduce dependencies when not needed. <sup>1</sup> Enabled by default.

| **Feature**             | **Description**                                                                               |
| ----------------------- | --------------------------------------------------------------------------------------------- |
| `map-file` <sup>1</sup> | Adds a convenience function to parse and map a FASTA/Q file to a provided index or reference. |
| `htslib`                | Supports returning results as HTS records.                                                    |
| `curl`                  | Enables curl support for htslib.                                                              |
| `simde`                 | Enables the SIMD Everywhere library in minimap2 compilation.                                  |
| `zlib-ng`               | Uses zlib-ng in the minimap2 index for faster reading of compressed files.                    |
| `static`                | Builds minimap2-rs as a static library in Rust.                                               |
| `sse2only`              | Builds minimap2 with only SSE2 support.                                                       |

Minimappers2 is an alternative to Mappy. It provides an interface to minimap2-rs via Python, returns results as high-performance data frames, and supports multithreading. This provides an alternative Pythonic interface to Minimap2, while adding native multi-threading. The Python interface is created using PyO3. The results are returned as a Polars data frame, and the function _.to_pandas()_ supports conversion to Pandas data frames. The number of threads to use is set with _.threads(n)_, where _n_ specifies the number of threads.

Minimap2-sys is the low-level direct FFI interface to the Minimap2 library, consisting primarily of unsafe function calls, automatically generated bindings from the bindgen crate, and some minor things to improve support and prevent memory leaks in downstream applications. The majority of the code here is in the build.rs file, which is responsible for building the Minimap2 C library, enabling architecture-specific features, and allowing Minimap2 to be built to support Rust interfacing while supporting additional platforms. The currently supported operating systems are Linux and Mac OS, as well as the architectures x86_64, aarch64, and arm64. Maximum portability of downstream binaries can be achieved by compiling for musl, an alternative libc implementation. These are tested with continuous integration to prevent any reversions or loss of system support. All features are also tested at this time. Experimental Android support is also tested on aarch64 and x86_64 via continuous integration. 

Fakeminimap2 provides further in-depth examples of multi-threading using both MPSC channels and Rayon. Further, Fakeminimap2 provides a terminal user-interface (TUI), allowing for interactive tables and visualizations of alignments (\ref{fakeminimap2-fig}). This TUI also supports mouse interaction in supported terminals. 

![Fakeminimap2 terminal user interface visualization example, aligning _E. coli_ isolate from Aquaculture farm Nanopore reads to _E. coli_ strain K-12 substr. MG1655 genome (). The list of query sequences is on the left, and navigable using the keyboard or the mouse. The list of alignments is on the right, listing some alignment statistics. Finally, a dot plot is found on the bottom right, showing a graphical representation of the alignment.](fakeminimap2.jpg)\label{fakeminimap2-fig}

# Results
This project provides an idiomatic and memory-safe interface from the popular systems-level Rust language to the widely used long-read mapping tool Minimap2. Minimap2-rs is amenable to multi-threading, which is essential for long reads and large genomes, and provides well-commented working examples of two different multi-threading approaches. With continuous integration, this will serve as a stable base for building Rust-based long-read sequencing applications.

# Acknowledgements
The author wishes to thank Wouter De Coster for kicking off this project with a tweet and Heng Li for creating Minimap2 and supporting this library. The author also thank the numerous contributors to GitHub, both through feature requests, finding bugs, and especially contributions via pull requests. Finally, a thank you to Peter Dearden for helping with manuscript prep and proof-reading.

# Conflict of Interest
None declared.

# Funding
This work was funded by a grant from Genomics Aotearoa (High Quality Genomes II), which is itself funded by the New Zealand Ministry of Business, Innovation and Employment. 

# Data Availability
The software repository is available on GitHub at https://github.com/jguhlin/minimap2-rs and all published versions of this crate are available at crates.io at https://crates.io/crates/minimap2. Minimappers2 is available through Pypi at https://pypi.org/project/minimappers2/. The software is available under the MIT or Apache 2.0 License, at the user's discretion. Fakeminimap2 data displayed used Nanopore reads from NCBI SRA SRR32385919, and whole genome sequence NCBI Genbank U00096.3.