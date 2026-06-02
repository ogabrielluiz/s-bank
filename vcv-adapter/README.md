# VCV adapter (Phase 7, placeholder)

This directory will hold the thin C++ `rack::Module` that links the Rust
`vactrol-core` staticlib over the C ABI in `crates/vactrol-core/src/ffi.rs`.
It is intentionally the last, optional phase: nothing here blocks the core
pipeline, which builds and tests with no Rack SDK.
