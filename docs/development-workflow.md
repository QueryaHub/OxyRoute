# Development workflow: branches and pull requests

OxyRoute uses **`dev` as the integration branch**. `main` (or the default production branch) is updated from `dev` when maintainers cut a release or merge a stable snapshot—follow what your team documents for release tagging.

## Branching model

1. **Always branch from the latest `dev`:**

   ```bash
   git fetch origin
   git checkout dev
   git pull origin dev
   ```

2. **Create a branch for one GitHub issue** (one topic per PR when possible):

   ```bash
   git checkout -b issue-12-rsgi-e2e
   # or: feat/12-rsgi-e2e  — use a short, kebab-case slug
   ```

   Prefer including the **issue number** in the name so the PR and issue stay linked (e.g. #12).

3. **Implement the issue** with **small, atomic commits** (one logical change per commit: `fix: …`, `test: …`, `docs: …`).

4. **Do not mix** code changes and **`.github/ISSUE_BACKLOG/`** (issue template bodies) in the same commit. The backlog is reference material for maintainers; if you need to update it, use a **separate** commit, e.g. `docs: update issue backlog description`.

5. **Push and open a pull request** with **base = `dev`** and **compare = your branch**.

   ```bash
   git push -u origin issue-12-rsgi-e2e
   ```

   On GitHub: **New pull request** → base repository `QueryaHub/OxyRoute`, **base: `dev`**, **compare: `issue-12-rsgi-e2e`**.

6. In the PR description, use **`Closes #N`** (or `Fixes #N`) if the work fully finishes issue **N**—GitHub will close the issue when the PR is merged.

7. After merge into `dev`, delete the remote branch (GitHub can do this on merge) and locally:  
   `git branch -d issue-12-rsgi-e2e` , then continue with the next issue from a fresh `dev`.

## What not to do

- Do **not** run `scripts/create-github-issues.sh` again unless you want **duplicate** issues on GitHub.
- Do **not** open a feature PR with base `main` unless the maintainers explicitly ask for a hotfix.

## `gh` CLI (optional)

```bash
gh pr create --base dev --head YOUR_BRANCH --title "..." --body "Closes #12"
gh pr list --base dev
```

[← Back to documentation index](index.md) · [Contributing (short)](../CONTRIBUTING.md)
