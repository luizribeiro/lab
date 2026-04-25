# tempo

Rust CLI for benchmarking LLM inference throughput (TTFT and decode tok/s)
over OpenAI-compatible streaming APIs.

Suites are described in TOML and expanded into a matrix of model × prompt
cells; each cell is run with optional warmup and N repetitions, and results
are emitted as a versioned JSON envelope.

Status: scaffolding. See the commit history for incremental progress.
