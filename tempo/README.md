# tempo

Rust CLI for benchmarking LLM inference throughput (TTFT and decode tok/s)
over OpenAI-compatible streaming APIs. Suites are described in TOML and
expanded into a matrix of model × prompt cells; each cell is run with optional
warmup and N repetitions, and results are emitted as a versioned JSON
envelope (`{"schema_version": 1, "rows": [...]}`).

## Build

```
nix build .#tempo
```

## Usage

```
cargo run --bin tempo -- run examples/vllm-smoke.toml -o results.json
```

The `-v` flag enables INFO-level tracing on stderr. The process exits with
status `1` if any matrix cell had zero successful runs.
