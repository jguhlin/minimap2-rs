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


```

# Want feedback
* Many fields are i32 / i8 to mimic the C environment, but would it make more sense to convert to u32 / u8 / usize?
* Let me know pain points

# Pain Points
Probably not freeing C memory somewhere.... expect crashes

# Next things todo
* Print other tags so we can have an entire PAF format
* Compile with SSE2 / SSE4.1 / SIMDe (auto-detect, but also make with features)
* Multi-thread guide (tokio async threads or use crossbeam queue and traditional threads?)
* Iterator interface for map_file

# Changelog
¯\_(ツ)_/¯