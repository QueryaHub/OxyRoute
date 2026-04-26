# Notes for AI agents (OxyRoute)

- **Git / branches / PRs / validation** — [`.cursor/rules/git-workflow.mdc`](.cursor/rules/git-workflow.mdc) (`alwaysApply`).
- **Local full check before push:** from repo root run `make test` (see [`Makefile`](Makefile)). Maintainers run this themselves before pushing; if a check fails, they will report it — then address that failure, rather than assuming everything passed without a run in your session.
