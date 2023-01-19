Python bindings for the [Rust FFI](https://github.com/jguhlin/minimap2-rs/) [minimap2](https://github.com/lh3/minimap2/) library. In development! Feedback appreciated!

# Why?
[PyO3](https://github.com/PyO3/pyo3) makes it very easy to create Python libraries via Rust. Further, we can use [Polars](https://github.com/pola-rs/polars) to export results as a dataframe (which can be used as-is, or converted to Pandas). Python allows for faster experimentation with novel algorithms, integration into machine learning pipelines, and provides an opportunity for those not familiar with Rust nor C/C++ to use minimap2.

## Why mininmappers2?
Because I'm terrible with names.

# How to use
## Requirements
tbd...

# Citation
You should cite the minimap2 papers if you use this in your work.

> Li, H. (2018). Minimap2: pairwise alignment for nucleotide sequences.
> *Bioinformatics*, **34**:3094-3100. [doi:10.1093/bioinformatics/bty191][doi]

and/or:

> Li, H. (2021). New strategies to improve minimap2 alignment accuracy.
> *Bioinformatics*, **37**:4572-4574. [doi:10.1093/bioinformatics/btab705][doi2]

# Changelog
## 0.1.0
* Initial Idea

# Funding
![Genomics Aotearoa](info/genomics-aotearoa.png)
