# Contributing to CactusOS

Thank you for your interest.

## Guidelines

1. Fork the repository and create a feature branch.
2. Keep changes focused and tested.
3. Do not commit secrets (`.env`, API keys).
4. Update documentation if you change behavior.
5. Open a pull request with a clear description.

## Development

```bash
# Python
python3 -m unittest discover -s tests 2>/dev/null || echo "No tests yet"

# Rust
cargo test
```

## Code Style

- Python: follow existing style in `si_1_core/`
- Rust: `no_std` friendly, no unnecessary allocations

## Questions

Open an issue.
