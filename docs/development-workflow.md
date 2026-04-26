# Development workflow: branches and pull requests

OxyRoute uses **`dev` as the integration branch**. `main` (or the default production branch) is updated from `dev` when maintainers cut a release or merge a stable snapshot—follow what your team documents for release tagging.

## Checklist (order matters)

1. **Sync `dev` first (always `fetch` + `pull` before branching):**
   ```bash
   git fetch origin
   git checkout dev
   git pull origin dev
   ```
2. **Create a branch** for one GitHub issue: `git checkout -b issue-<N>-<short-slug>`.
3. **Read the issue** (and [`.github/ISSUE_BACKLOG/bodies/`](.github/ISSUE_BACKLOG/bodies/) or linked spec if you use it).
4. **Implement** on that branch.
5. **Run tests and linters** (same as CI, see [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)):
   - `uv run ruff check oxyroute tests examples` and `uv run ruff format --check oxyroute tests examples`
   - `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`
   - `uv run pytest` (or the isolated pattern in [development.md](development.md))
6. **Commit atomically** (one logical change per commit: `feat:`, `fix:`, `test:`, `docs:`, …).
7. **Push and open a PR to `dev`** (base = `dev`), with **`Closes #N`** when the issue is done.

## Branch naming

- Prefer `issue-<N>-<kebab-slug>` or `feat/<N>-<kebab>` so the PR links to the issue (e.g. #12).

## Atomic commits and backlog

- **Do not mix** product code and **`.github/ISSUE_BACKLOG/`** in the same commit. If you update templates, use a **separate** `docs:` or `chore:` commit (or a separate PR).

## Push and open a pull request

```bash
git push -u origin issue-12-rsgi-e2e
```

On GitHub: **New pull request** → **base: `dev`**, **compare: your branch**. Use **`Closes #N`** (or `Fixes #N`) to auto-close the issue on merge.

## After merge

Delete the remote branch (GitHub can do this on merge) and locally: `git branch -d issue-12-rsgi-e2e`. For the **next** issue, repeat from **fetch + pull `dev`**.

## What not to do

- Do **not** run `scripts/create-github-issues.sh` again unless you want **duplicate** issues on GitHub.
- Do **not** open a feature PR with base `main` unless the maintainers explicitly ask for a hotfix.

## `gh` CLI (optional)

```bash
gh pr create --base dev --head YOUR_BRANCH --title "..." --body "Closes #12"
gh pr list --base dev
```

[← Back to documentation index](index.md) · [Contributing (short)](../CONTRIBUTING.md)
