# Multithreaded example of mapping

Using crossbeam and standard library threading, we can multi thread alignments sharing the same references to the index. 
Small example files are provided, 

## Logging
Logging is done using the log crate, and the log level can be set using the RUST_LOG environment variable.

```
RUST_LOG=info cargo run
```