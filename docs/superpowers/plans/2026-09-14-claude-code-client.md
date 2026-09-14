# Claude Code SVN Client Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a Windows/Linux `svn-ai` command-line client that automatically records Claude Code Edit and Write changes, stores fingerprint-only attribution events locally, and calculates statistics after those changes are committed to SVN.

**Architecture:** Claude Code invokes one non-blocking `svn-ai hook` command for PreToolUse, PostToolUse, and PostToolUseFailure. The client discovers the enclosing SVN working copy through the Subversion 1.7-compatible `svn info --xml` interface, stores short-lived before-content snapshots and finalized attribution events in local SQLite, and reuses the existing `svnlook` revision reader and matcher for diagnostic statistics. Hook installation merges entries into an existing Claude settings file without deleting unrelated settings.

**Tech Stack:** Rust 1.85.1, Clap, Rusqlite with bundled SQLite, Quick XML, Serde JSON, Claude Code command hooks, Subversion 1.7-compatible CLI commands.

**Spec:** `docs/superpowers/specs/2026-09-08-svn-ai-attribution-system-design.md`

## Global Constraints

- The client supports Windows and Linux command-line environments and must be packageable for offline installation.
- The first executable client increment supports Claude Code `Edit` and `Write`; `Bash` filesystem coverage is explicitly deferred to the next client increment.
- Hook processing must never make Claude Code deny or block a tool operation; capture errors are written locally and the hook process returns success.
- Finalized event JSON contains keyed line fingerprints and repository-relative paths, never prompts, responses, transcripts, absolute paths, or complete source files.
- Short-lived before-content snapshots remain only in the local SQLite transaction table and are deleted on PostToolUse or PostToolUseFailure.
- SVN discovery uses `svn info --xml --depth empty`, which is documented in Subversion 1.7; it does not read or modify `.svn/wc.db` directly.
- This phase provides local post-commit statistics for validation. Automatic upload, VisualSVN queue processing, and centralized Linux statistics remain separate implementation phases.

---

### Task 1: Local client configuration and SQLite event store

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/svn-ai-client/Cargo.toml`
- Create: `crates/svn-ai-client/src/lib.rs`
- Create: `crates/svn-ai-client/src/config.rs`
- Create: `crates/svn-ai-client/src/store.rs`
- Test: `crates/svn-ai-client/tests/config_store.rs`

**Interfaces:**
- Produces: `ClientConfig::{load, save}`, `FingerprintSecret::parse`, `EventStore::{open, begin_transaction, finish_transaction, cancel_transaction, list_events, status}`.
- Persists: `transactions(session_digest, tool_digest, before_content, repository metadata, file metadata)` and `attribution_events(event_id, event_json, created_at)`.

- [x] **Step 1: Write failing tests** proving configuration round-trips, invalid 64-character keys are rejected, transactions survive process reopen, finishing atomically deletes the snapshot and inserts one event, duplicate event IDs are idempotent, and cancellation deletes source snapshots.
- [x] **Step 2: Run `cargo test -p svn-ai --test config_store`** and confirm failure because the client crate does not exist.
- [x] **Step 3: Implement configuration and SQLite schema** using a caller-supplied home directory so production and tests have identical behavior.
- [x] **Step 4: Run `cargo test -p svn-ai --test config_store`** and confirm every storage test passes.
- [x] **Step 5: Commit** with message `feat: add local SVN AI client store`.

### Task 2: Subversion working-copy discovery

**Files:**
- Modify: `crates/svn-ai-svn/Cargo.toml`
- Create: `crates/svn-ai-svn/src/client.rs`
- Modify: `crates/svn-ai-svn/src/lib.rs`
- Test: `crates/svn-ai-svn/tests/client.rs`

**Interfaces:**
- Consumes: the existing `CommandRunner` boundary.
- Produces: `SvnClient::{new, with_runner, discover}` and `WorkingCopyInfo { root_path, repository_uuid, repository_root_url, repository_path_prefix, base_revision }`, plus `WorkingCopyInfo::repo_path(&Path)`.

- [x] **Step 1: Write failing tests** using real Subversion 1.7 XML fixtures for normal roots, nested targets, URL escaping, paths containing spaces/non-ASCII text, files not yet created by Write, targets outside the working copy, command errors, and malformed XML.
- [x] **Step 2: Run `cargo test -p svn-ai-svn --test client`** and confirm failure because `SvnClient` is missing.
- [x] **Step 3: Implement upward working-copy discovery and XML parsing** with exact `svn info --xml --depth empty <target>` argument tests.
- [x] **Step 4: Run `cargo test -p svn-ai-svn --test client`** and confirm all discovery tests pass.
- [x] **Step 5: Commit** with message `feat: discover SVN working copies`.

### Task 3: Claude Code hook transaction lifecycle

**Files:**
- Create: `crates/svn-ai-client/src/hook.rs`
- Modify: `crates/svn-ai-client/src/lib.rs`
- Test: `crates/svn-ai-client/tests/hook.rs`

**Interfaces:**
- Consumes: Claude hook JSON, `ClientConfig`, `EventStore`, `SvnClient`, `diff_files`, and protocol `AttributionEvent`.
- Produces: `HookInput`, `HookProcessor::process`, and `HookOutcome::{Captured, Finalized, Cancelled, Ignored}`.

- [x] **Step 1: Write failing tests** for official PreToolUse/PostToolUse/PostToolUseFailure JSON shapes, Edit/Write target extraction, a missing file before Write, a ten-line change, failed-tool cancellation, unsupported tools, outside-SVN files, and event JSON privacy.
- [x] **Step 2: Run `cargo test -p svn-ai --test hook`** and confirm failure because hook processing is missing.
- [x] **Step 3: Implement the lifecycle** keyed by digests of `session_id` and `tool_use_id`, reading file contents only before and after the tool and emitting protocol events only after success.
- [x] **Step 4: Run `cargo test -p svn-ai --test hook`** and confirm all hook tests pass.
- [x] **Step 5: Commit** with message `feat: capture Claude Code file edits`.

### Task 4: Hook installer and client CLI

**Files:**
- Create: `crates/svn-ai-client/src/settings.rs`
- Create: `crates/svn-ai-client/src/main.rs`
- Modify: `crates/svn-ai-client/src/lib.rs`
- Test: `crates/svn-ai-client/tests/settings.rs`
- Test: `crates/svn-ai-client/tests/cli.rs`

**Interfaces:**
- Produces commands: `svn-ai configure`, `svn-ai install-hooks`, `svn-ai hook`, `svn-ai status --json`, `svn-ai events --json`, and `svn-ai stats --repository <PATH> --revision <N> --svnlook <PATH> --json`.
- Installs three command hooks with matcher `Edit|Write`: PreToolUse, PostToolUse, and PostToolUseFailure.

- [x] **Step 1: Write failing settings tests** proving unrelated Claude settings and hooks are preserved, exact duplicate SVN AI hooks are not added twice, executable/home paths with spaces are safely quoted, and a backup is created before modifying an existing file.
- [x] **Step 2: Write failing CLI tests** proving configure/status/events output, settings installation, and that malformed stdin hook input exits zero with a sanitized recorded diagnostic. The real `stats` command boundary is exercised in Task 5 because it requires a repository rather than a command mock.
- [x] **Step 3: Run `cargo test -p svn-ai --test settings --test cli`** and confirm failures because the installer and executable are absent.
- [x] **Step 4: Implement settings merge and CLI commands** without changing the user's real Claude settings during tests.
- [x] **Step 5: Run `cargo test -p svn-ai --test settings --test cli`** and confirm all client command tests pass.
- [x] **Step 6: Commit** with message `feat: add Claude Code hook installer and client CLI`.

### Task 5: Real SVN automatic-capture acceptance test

**Files:**
- Create: `tests/e2e/claude_hook_svn_round_trip.sh`
- Create: `scripts/verify-claude-svn-e2e.ps1`
- Modify: `README.md`

**Interfaces:**
- Consumes: the production `svn-ai` executable, real `svn`, `svnadmin`, and `svnlook` commands, and official Claude Code hook JSON shapes.
- Produces: a repeatable Docker test whose final JSON is `svn_additions=10`, `ai_additions=8`, `non_ai_additions=2`, `ambiguous_additions=0`.

- [x] **Step 1: Write the failing shell acceptance test** that configures the client, installs hooks into a temporary settings file, sends PreToolUse, modifies ten lines, sends PostToolUse, manually rewrites two lines, commits revision 2, and runs `svn-ai stats`.
- [x] **Step 2: Run `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-claude-svn-e2e.ps1`** and confirm the test fails before all CLI behavior exists.
- [x] **Step 3: Complete only the wiring exposed by the acceptance failure**, keeping source snapshots local and verifying the finalized event file contains no source text.
- [x] **Step 4: Re-run the acceptance test** and confirm the real SVN revision reports 10 total additions, 8 AI additions, 2 non-AI additions, and 0 ambiguous additions.
- [x] **Step 5: Update README** with installation, configuration, Claude settings, local validation steps, and explicit limitations for Bash and centralized aggregation.
- [x] **Step 6: Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, the previous SVN round trip, and the new Claude-hook SVN round trip.**
- [x] **Step 7: Commit** with message `test: verify automatic Claude to SVN attribution`.
