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
      affiliation: "1"
affiliations:
    - name: Genomics Aotearoa, Department of Biochemistry, University of Otago, Dunedin, New Zealand
      index: 1
date: 4 May 2026
bibliography: paper.bib

---

# Summary
Long-read sequence alignment underpins modern genomic research, enabling highly contiguous de novo assemblies, structural variant detection, RNA isoform exploration, and many other applications. Minimap2, the long-read sequence aligner, sits at the center of these developments, providing fast alignment for long and noisy reads. The Rust programming language has seen rapid adoption in the scientific community, offering memory safety, concurrency, high performance, and simplified dependency management. Here, we present Minimap2-rs, a collection of Rust crates providing a Rust-centric interface to Minimap2, lowering the barrier to using this aligner in Rust-based applications.

The top-level Minimap2 crate offers an opinionated, flexible, idiomatic API that manages memory and simplifies calling C functions via the Rust foreign function interface (FFI). A companion Python library, minimappers2, provides multi-threading and returns high-performance dataframes. The minimap2-sys crate provides direct, low-level bindings to Minimap2 for specialized use cases. This modular design facilitates rapid prototyping and production-ready pipelines. Continuous integration across x86_64 and arm64 architectures ensures reliability on Linux, macOS, and Android, with tested support for musl-based portability. Minimap2-rs has been adopted by multiple tools for genome size estimation, tandem repeat genotyping, and transcriptome analysis. Minimap2-rs offers a stable, open-source foundation for building Rust-powered bioinformatics solutions while preserving Minimap2’s speed and flexibility.

# Statement of Need
Long-read sequence alignment has changed the foundation of recent genomic advances. Thanks to long-read sequence alignment, advances such as the telomere-to-telomere Human genome assembly [@nurk2022complete] were completed, the cost to assemble a _de novo_ genome has decreased significantly, contiguity has increased [@van2023nanopore; @koren2015one; @murigneux2020comparison; @hifiasm; @flye], and it is now used for RNA-based applications [@jain2022advances]. The primary algorithm supporting nearly all of these advancements is Minimap2, which aligns long-read sequences to large references while accounting for the high error rate in long reads [@minimap2; @minimap2new]. Minimap2 can also handle spliced, short-read, and assembly-to-assembly alignments.

The Rust programming language has seen significant adoption, including amongst scientists [@rust; @bugden2022rust; @rustscience]. Rust brings memory safety, strong ownership, and performance to applications and libraries, with consistent build tooling [@bugden2022rust]. This ultimately enables highly portable applications with strong concurrency idioms, encouraging high-performance by default. With the growing adoption of Rust, there are now several tools and libraries in Rust for bioinformatics [@huey2024bigtools; @buffalo2024granges; @rustbio; @chan2024next].

# State of the Field
Minimap2 is a command-line tool and library, and has an official Python binding, Mappy. Using Minimap2 directly in Rust requires either foreign function interface (FFI) calls or running Minimap2 as a subprocess, which may not be ideal for situations that require custom workflows. Minimap2-rs provides FFI bindings to the Minimap2 library, exposing the underlying API directly to Rust and supporting an idiomatic Rust API that works with the Rust ownership model, simplifying the interface with Minimap2 from Rust. Minimap2-rs is open source and publicly available through crates.io, the standard library registry for Rust, and GitHub.

# Software Design
Minimap2 uses single-instruction multiple-data (SIMD) CPU features to achieve higher performance and better parallelization, enabling it to handle large datasets and long reads. Minimap2-rs consists of three libraries (called 'crates' in Rust): Minimap2, minimappers2, and minimap2-sys. The idiomatic entry point is the Minimap2 Rust crate (further referred to as minimap2-rs), which provides an opinionated, memory-safe API for working with Minimap2 functions via the foreign function interface (FFI) and returning results in Rust data structures (structs), automatically converting base types between C and Rust. Minimappers2 is a Python binding wrapping minimap2-rs, and an alternative to minimap2’s own Mappy, providing built-in multi-threading and returning results as high-performance, memory-efficient data frames. In Rust, there is a convention of using a separate crate to wrap the C library, and that convention is followed here: Minimap2-sys wraps the C library and provides a direct interface to the C functions in Rust via unsafe code [@rust]. A functional fourth tool, fakeminimap2, is available and serves as a coding exemplar of two common multithreading approaches.

Minimap2-rs is the primary interface for using Minimap2 in downstream Rust applications. An aligner struct is created, and presets, mapping, and indexing options are configured within it. All presets from the Minimap2 command-line software (e.g., map-ont, map-pb, map-hifi, and asm20), as well as low-level access to additional index and mapping options in the Minimap2 C library, are available. The crate supports optional features (see \ref{crate-features}). Minimap2 is built using Rust’s tooling; thus, adding this library only requires adding it as a dependency. The code repository provides two examples of multithreading implementation with minimap2-rs: a multi-producer single-consumer (MPSC) channel and parallelization using the popular Rayon crate.

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


Minimap2-sys is the low-level, direct FFI interface to the Minimap2 library, containing most unsafe function calls, with automatically generated bindings from the bindgen crate, and minor improvements to prevent memory leaks in downstream applications. The majority of the code here is in the build.rs file, which builds the Minimap2 C library, enables architecture-specific features, and allows Minimap2 to link to Rust. Compatible operating systems for minimap2-rs are Linux and Mac OS, on supported architectures x86_64, aarch64, and arm64. Maximum portability of downstream binaries can be achieved by compiling for musl, an alternative libc implementation. These are tested with continuous integration to prevent any regressions or loss of system support. All features are tested at this time. Experimental Android support is also tested on aarch64 and x86_64.

Fakeminimap2 provides in-depth examples of multi-threading using both MPSC channels and Rayon, as well as a terminal user interface (TUI) that supports interactive tables and visualizations of alignments (see \ref{fakeminimap2-fig}), and handles mouse interaction events in supported terminals.

![Fakeminimap2 terminal user interface visualization example, aligning _E. coli_ isolate from Aquaculture farm Nanopore reads to _E. coli_ strain K-12 substr. MG1655 genome (NCBI Genbank accession U00096.3). The list of query sequences is on the left, and navigable using the keyboard or the mouse. The list of alignments is on the right, listing some alignment statistics. Finally, a dot plot is found on the bottom right, showing a graphical representation of the alignment.](fakeminimap2.jpg)\label{fakeminimap2-fig}

# Research Impact Statement
Minimap2-rs is already used by multiple tools for sequence cleaning [@de2023nanopack2], tandem-repeat genotyping [@strdust], long-read transcriptome quantification [@oarfish], long-read-based genome size estimation [@hall_genome_2024], pharmacogenomic diplotyping [@starphase], and genotyping for large-unit variable number tandem repeats [@kivvi]. As of this publication, there are 16 additional, non-automated contributors to the repo.

# AI Usage Disclosure
This library was primarily written manually, with generative AI used only for some recent bugs and for exploring features in branches not part of the release. All generated code was manually reviewed and tested against the existing test suite, which also verifies correctness against the official minimap2 binary. Generative coding AI was also used to expand the C to Rust mapping options and explanations found in the README.

# Acknowledgements
The author wishes to thank Wouter De Coster for kicking off this project with a tweet and Heng Li for creating Minimap2 and supporting this library. The author also thanks the numerous contributors to GitHub, both for feature requests, bug reports, and especially for pull requests. Finally, a thank you to Peter Dearden for helping with manuscript prep and proofreading.

# Conflict of Interest
None declared.

# Funding
This work was funded by a grant from Genomics Aotearoa (High Quality Genomes II), which is funded by the New Zealand Ministry of Business, Innovation and Employment.

# Data Availability
The software repository is available on GitHub at https://github.com/jguhlin/minimap2-rs, and all published versions of this crate are available at crates.io at https://crates.io/crates/minimap2. Minimappers2 is available through PyPI at https://pypi.org/project/minimappers2/. The software is available under the MIT or Apache 2.0 License, at the user’s discretion. Fakeminimap2 data displayed used Nanopore reads from NCBI SRA SRR32385919 and the whole-genome sequence NCBI GenBank U00096.3.