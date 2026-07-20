# Notes for AI agents (OxyRoute)

- **Git / branches / PRs / validation** — [docs/development-workflow.md](docs/development-workflow.md) (integration branch `dev`, issue branches, atomic commits, PR to `dev`).
- **Validation (ruff, cargo, `make test`, pytest, maturin):** the **maintainer runs these** before push. **Do not** run that full pipeline in the agent unless the user explicitly asks (e.g. “run `make test`”, “fix the failing test”). If something failed in CI or locally, the user will say so — then fix what they report.
