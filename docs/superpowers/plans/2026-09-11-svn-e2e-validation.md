# SVN End-to-End Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and run a repeatable vertical-slice test proving that a real Subversion commit can be read with `svnlook` and matched to previously recorded AI-generated lines without transmitting source code.

**Architecture:** Add a reusable Rust adapter around the Subversion 1.7-compatible `svnlook` command set, then add line-level attribution matching and a small validation CLI. A Docker-based test creates a real repository, records ten AI lines, changes two lines manually, commits through `svn`, and verifies that eight lines are attributed to AI and two are not.

**Tech Stack:** Rust 1.85.1, Subversion command-line tools (`svn`, `svnadmin`, `svnlook`), Docker, Bash, Serde, Clap.

**Spec:** `docs/superpowers/specs/2026-09-08-svn-ai-attribution-system-design.md`

## Global Constraints

- Target server compatibility is VisualSVN Server Standard Edition 2.5.2 with Apache Subversion 1.7.2.
- Only the `svnlook author`, `date`, `uuid`, `changed --copy-info`, and `cat` commands documented for Subversion 1.7 may be used.
- Uploaded or persisted validation artifacts contain keyed line fingerprints and repository-relative paths, never prompts, responses, or complete source files.
- Production code is written only after the corresponding test has failed for the expected reason.
- The validation CLI is diagnostic scaffolding; the `svn-ai-svn` library must be reusable by the later Windows Hook Agent.

---

### Task 1: Parse Subversion changed-path output

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/svn-ai-svn/Cargo.toml`
- Create: `crates/svn-ai-svn/src/lib.rs`
- Create: `crates/svn-ai-svn/src/changed.rs`
- Test: `crates/svn-ai-svn/tests/changed.rs`

**Interfaces:**
- Consumes: `svn_ai_protocol::{ChangeKind, CopyFrom, RepoPath}`.
- Produces: `parse_changed(&str) -> Result<Vec<ChangedPath>, ChangedParseError>` and `ChangedPath { path, kind, copy_from, text_changed, properties_changed }`.

- [x] **Step 1: Write failing parser tests** covering add, modify, delete, property-only changes, paths containing spaces, copy-from records, directories, and malformed copy metadata.
- [x] **Step 2: Run `cargo test -p svn-ai-svn --test changed`** and confirm failure because the crate/API does not exist.
- [x] **Step 3: Implement the minimal status-column and copy-info parser** without locale-dependent prose except the stable `(from PATH:rREV)` record emitted by Subversion 1.7.
- [x] **Step 4: Run `cargo test -p svn-ai-svn --test changed`** and confirm all parser tests pass.
- [x] **Step 5: Commit** with message `feat: parse svnlook changed output`.

### Task 2: Read a real revision through svnlook

**Files:**
- Create: `crates/svn-ai-svn/src/command.rs`
- Create: `crates/svn-ai-svn/src/revision.rs`
- Test: `crates/svn-ai-svn/tests/revision.rs`

**Interfaces:**
- Consumes: `parse_changed`, `svn_ai_core::diff_files`, `FingerprintKey`.
- Produces: `SvnLook::new(PathBuf)`, `SvnLook::read_revision(&Path, i64, &FingerprintKey) -> Result<RevisionEvent, SvnLookError>`.

- [x] **Step 1: Write failing tests** using a deterministic command fixture that exercises exact argument construction, UTF-8 path handling, add/modify/delete content lookup, UUID/author/date extraction, and non-zero command exits.
- [x] **Step 2: Run `cargo test -p svn-ai-svn --test revision`** and confirm failure because the reader is missing.
- [x] **Step 3: Implement the command boundary and revision reader** using only the Subversion 1.7-compatible commands listed in the global constraints.
- [x] **Step 4: Run `cargo test -p svn-ai-svn --test revision`** and confirm all reader tests pass.
- [x] **Step 5: Commit** with message `feat: read svn revisions with svnlook`.

### Task 3: Match recorded AI lines to committed SVN lines

**Files:**
- Create: `crates/svn-ai-core/src/matching.rs`
- Modify: `crates/svn-ai-core/src/lib.rs`
- Test: `crates/svn-ai-core/tests/matching.rs`

**Interfaces:**
- Consumes: protocol `AttributionEvent` and `RevisionFileChange` fingerprints.
- Produces: `match_attribution(&RevisionEvent, &[AttributionEvent]) -> AttributionMetrics` with total additions, AI additions, non-AI additions, and ambiguous additions.

- [x] **Step 1: Write failing tests** for an exact ten-line match, two manually rewritten lines, unrelated paths/users/repositories, duplicate-line ambiguity, and one-time event consumption.
- [x] **Step 2: Run `cargo test -p svn-ai-core --test matching`** and confirm failure because the matcher is missing.
- [x] **Step 3: Implement deterministic one-to-one matching** by repository UUID, SVN username, repository path, base revision, line digest, and context digest; unresolved duplicates are ambiguous rather than AI.
- [x] **Step 4: Run `cargo test -p svn-ai-core --test matching`** and confirm all matcher tests pass.
- [x] **Step 5: Commit** with message `feat: match AI lines to SVN revisions`.

### Task 4: Add and execute the real SVN validation harness

**Files:**
- Create: `crates/svn-ai-validate/Cargo.toml`
- Create: `crates/svn-ai-validate/src/main.rs`
- Create: `tests/e2e/svn_round_trip.sh`
- Create: `scripts/verify-svn-e2e.ps1`
- Modify: `README.md`

**Interfaces:**
- Consumes: `SvnLook::read_revision`, `diff_files`, protocol events, and `match_attribution`.
- Produces: `svn-ai-validate capture` and `svn-ai-validate verify --json`; the PowerShell entry point runs the complete Docker-hosted SVN scenario.

- [x] **Step 1: Write the shell acceptance test first** so it calls the not-yet-existing CLI and asserts literal JSON metrics: `svn_additions=10`, `ai_additions=8`, `non_ai_additions=2`, `ambiguous_additions=0`.
- [x] **Step 2: Run `powershell -ExecutionPolicy Bypass -File scripts/verify-svn-e2e.ps1`** and confirm failure because the validation CLI is absent.
- [x] **Step 3: Implement the CLI** so `capture` emits fingerprint-only attribution JSON and `verify` reads the real repository revision with `svnlook`, validates identity, and emits metrics JSON.
- [x] **Step 4: Run the PowerShell entry point** and confirm a real `svnadmin create` / `svn checkout` / `svn commit` round trip returns the four expected metrics.
- [x] **Step 5: Update README** with current scope, exact commands, expected output, and the remaining requirement for a final smoke test on the bank's VisualSVN 2.5.2 host.
- [x] **Step 6: Run full verification:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and the Docker SVN round trip.
- [x] **Step 7: Commit** with message `test: validate attribution through a real SVN commit`.
