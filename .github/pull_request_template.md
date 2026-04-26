## Summary

- What this PR does (1–3 sentences).
- If it closes a ticket: `Closes #N` (replace N).

## Base branch

- [ ] This PR targets **`dev`**, not `main` (unless maintainers asked for a hotfix).

## Checklist

- [ ] `cargo build` (and `cargo clippy` if you touched Rust)
- [ ] `maturin develop` + tests as in [docs/development.md](docs/development.md) (e.g. pytest from a clean cwd / installed wheel)
- [ ] No unrelated drive-by refactors; commits are [atomic and scoped](https://github.com/QueryaHub/OxyRoute/blob/main/docs/development-workflow.md)

## Note on `.github/ISSUE_BACKLOG/`

If you only touch issue template files under [`.github/ISSUE_BACKLOG/`](.github/ISSUE_BACKLOG/) (not application code), say so in the summary. Prefer a **separate** PR for backlog-only edits when possible.
