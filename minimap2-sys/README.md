# System Bindings for libminimap2
Use this if you need lower-level bindings for minimap2.

## TODO
* Add SIMDe support
* Can we decouple from pthread?

## Changelog

### 0.1.9 IN DEVELOPMENT
* HTS lib support by @eharr
* HTS Lib: Output sam/bam files
* More tests by @eharr
* SSE2/4 support can be enabled by using the "sse" feature
* Update minimap2-sys to latest version


### 0.1.8
* Changed how zlib is compiled
* Dep versions update
* Added SSE compilation feature (Mostly autodetects)

### 0.1.7
* Make bindgen an optional feature
* zlib support for musl builds