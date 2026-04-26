# tempo

Rust CLI for benchmarking LLM inference throughput (TTFT and decode tok/s)
over OpenAI-compatible streaming APIs. Suites are described in TOML and
expanded into a matrix of arbitrary user-defined axes (with reserved
conventional names `model`, `prompt`, `max_tokens`, `temperature`, `top_p`);
each cell is run with optional warmup and N repetitions, and results are
emitted as a versioned JSON envelope:

```json
{
  "schema_version": 2,
  "rows": [
    {
      "suite": "...",
      "scenario": "decode",
      "provider": "litellm",
      "vars": { "model": "...", "prompt": "short", "max_tokens": 2048 },
      "run_idx": 0,
      "started_at": "...",
      "ttft_ms": 76.0,
      "decode_tok_s": 88.0,
      "e2e_ms": 365.0,
      "input_tokens": 26,
      "output_tokens": 23,
      "error": null
    }
  ]
}
```

All matrix axes — including `model` and `prompt` — appear nested under
`vars`. There are no top-level `model` / `prompt` fields.

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
