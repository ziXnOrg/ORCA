# Rule: Python Standards (SDK/Tools)

- Target Python 3.11+; `pyproject.toml` for tooling.
- Lint: `ruff` as errors; Format: `black` (line length 100); Type: `mypy --strict`.
- Testing: `pytest` with coverage ≥85%, deterministic seeds.
- Packaging: explicit dependencies pinned via `uv`/`pip-tools` or Poetry with lockfiles.
- Runtime: no blocking I/O in async; structured logging; no prints in library code.
- Security: no secrets in code; environment/secret managers; redact logs; validate all inputs.