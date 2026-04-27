# tempo

CLI for benchmarking LLM inference throughput (TTFT and decode tok/s)
over OpenAI-compatible streaming APIs. Suites are described in TOML and
expanded into a matrix of user-defined axes; each cell is run with
optional warmup and N repetitions, and results are emitted as JSON.

## Install

Homebrew (macOS, Linux):

```
brew install luizribeiro/tap/tempo
```

Or grab a prebuilt binary from the [latest release][releases] (macOS and
Linux, x86_64 and arm64).

[releases]: https://github.com/luizribeiro/lab/releases?q=tempo

## Usage

Run a suite:

```
tempo run my-suite.toml -o results.json
```

Re-render the summary table from an existing results file (no re-runs):

```
tempo report results.json
```

`-v` enables INFO-level tracing on stderr. `tempo run` exits non-zero if
any matrix cell had zero successful runs.

### Suite file

```toml
[suite]
name = "vllm-smoke"

[providers.vllm]
kind        = "openai_compatible"
base_url    = "http://litellm.internal/v1"
api_key_env = "LITELLM_KEY"

[prompts.short]
kind = "inline"
text = "Write a haiku about caching."

[scenarios.decode]
kind     = "throughput"
provider = "vllm"
warmup   = 1
runs     = 5
generation = { max_tokens = 256, temperature = 0.0 }

[scenarios.decode.matrix]
model  = ["llama-3.1-70b", "qwen2.5-72b"]
prompt = ["short"]
```

The matrix expands into one cell per combination. Reserved axis names
are `model`, `prompt`, `max_tokens`, `temperature`, `top_p`; any other
axis name is treated as a user-defined variable. See
`examples/vllm-smoke.toml` and `examples/multiaxis-demo.toml` for more.

### Output

Results are written as a versioned JSON envelope:

```json
{
  "schema_version": 2,
  "rows": [
    {
      "suite": "vllm-smoke",
      "scenario": "decode",
      "provider": "vllm",
      "vars": { "model": "llama-3.1-70b", "prompt": "short", "max_tokens": 256 },
      "run_idx": 0,
      "started_at": "2026-04-27T01:00:00Z",
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

All matrix axes — including `model` and `prompt` — are nested under
`vars`. There are no top-level `model` / `prompt` fields.

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for development setup and how
to build from source.
