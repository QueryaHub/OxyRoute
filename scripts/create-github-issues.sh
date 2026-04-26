#!/usr/bin/env bash
# One-time: creates milestone v0.2.0, labels, and 20 GitHub issues for OxyRoute.
# QueryaHub/OxyRoute has already run this; use README ISSUE_BACKLOG for current milestones (v0.3.0 follow-ups).
# Prerequisite: `gh auth login` with issues:write on the repository.
# Re-running duplicates issues; only use on a clean tracker or a fork.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BODIES="$ROOT/.github/ISSUE_BACKLOG/bodies"

if ! command -v gh &>/dev/null; then
  echo "Error: install GitHub CLI: https://cli.github.com/" >&2
  exit 1
fi
if ! gh auth status &>/dev/null; then
  echo "Error: run: gh auth login" >&2
  exit 1
fi

REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null || true)
if [[ -z "$REPO" ]]; then
  echo "Error: run this from a clone with a gh default repo, or use: gh repo set-default OWNER/REPO" >&2
  exit 1
fi

create_label() {
  local n="$1" c="${2:-ededed}"
  gh label create "$n" --color "$c" 2>/dev/null || true
}

create_label "P0" "B60205"
create_label "P1" "FBCA04"
create_label "research" "0E8A16"
create_label "tech-debt" "D93F0B"
create_label "documentation" "0075CA"
create_label "test" "BFD4F2"
create_label "ci" "1D76DB"
create_label "enhancement" "A2EEEF"
create_label "bug" "D73A4A"

# Milestone
gh api "repos/${REPO}/milestones" -f title="v0.2.0" -f state="open" 2>/dev/null || true

M=( -m "v0.2.0" )

gh issue create -R "$REPO" --title "feat: expose PATCH routes on Python App" --body-file "$BODIES/01.md" -l enhancement -l P1 "${M[@]}"
gh issue create -R "$REPO" --title "fix: apply URL decoding to query string keys and values" --body-file "$BODIES/02.md" -l bug -l P0 "${M[@]}"
gh issue create -R "$REPO" --title "feat: map Python exceptions in dispatch to HTTP 500 with safe body" --body-file "$BODIES/03.md" -l enhancement -l P0 "${M[@]}"
gh issue create -R "$REPO" --title "perf: reduce lock contention on route match (RWLock or route snapshot)" --body-file "$BODIES/04.md" -l enhancement -l "tech-debt" "${M[@]}"
gh issue create -R "$REPO" --title "feat: pass request context into dependencies and support dependency chains" --body-file "$BODIES/05.md" -l enhancement -l P1 "${M[@]}"
gh issue create -R "$REPO" --title "feat: extend JWT Validation with iss/aud and optional leeway" --body-file "$BODIES/06.md" -l enhancement -l P1 "${M[@]}"
gh issue create -R "$REPO" --title "feat: read JWT from Cookie header for require_jwt" --body-file "$BODIES/07.md" -l enhancement "${M[@]}"
gh issue create -R "$REPO" --title "research+feat: JWK/PEM-based JWT verify in Rust (align with oxyjwt)" --body-file "$BODIES/08.md" -l enhancement -l research "${M[@]}"
gh issue create -R "$REPO" --title "feat: add optional Pydantic / JSON schema hooks for OpenAPI body" --body-file "$BODIES/09.md" -l enhancement "${M[@]}"
gh issue create -R "$REPO" --title "feat: structured Response object for headers and status" --body-file "$BODIES/10.md" -l enhancement -l P1 "${M[@]}"
gh issue create -R "$REPO" --title "feat: optional middleware chain in run_rsgi before route match" --body-file "$BODIES/11.md" -l enhancement "${M[@]}"
gh issue create -R "$REPO" --title "test: e2e HTTP against granian --interface rsgi" --body-file "$BODIES/12.md" -l test -l P0 "${M[@]}"
gh issue create -R "$REPO" --title "ci: add cargo clippy and Rust unit tests to workflow" --body-file "$BODIES/13.md" -l ci "${M[@]}"
gh issue create -R "$REPO" --title "ci: PyPI publish on version tag (trusted publishing)" --body-file "$BODIES/14.md" -l ci "${M[@]}"
gh issue create -R "$REPO" --title "tech-debt: upgrade pyo3 0.21 -> current and validate Granian RSGI" --body-file "$BODIES/15.md" -l "tech-debt" -l research "${M[@]}"
gh issue create -R "$REPO" --title "docs+test: clarify include_openapi vs set_openapi_served and 404 for /openapi.json" --body-file "$BODIES/16.md" -l documentation -l test "${M[@]}"
gh issue create -R "$REPO" --title "fix: review ASGI protocol bridge (run_coroutine_threadsafe) under load" --body-file "$BODIES/17.md" -l bug -l research "${M[@]}"
gh issue create -R "$REPO" --title "feat: pass app state / lifespan for shared resources (optional design)" --body-file "$BODIES/18.md" -l enhancement "${M[@]}"
gh issue create -R "$REPO" --title "feat: return 405 when path exists for another method" --body-file "$BODIES/19.md" -l enhancement "${M[@]}"
gh issue create -R "$REPO" --title "feat: register HEAD/OPTIONS and sensible defaults" --body-file "$BODIES/20.md" -l enhancement "${M[@]}"

echo "Done. Open: https://github.com/${REPO}/issues"
