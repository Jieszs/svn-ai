# SVN AI Phase 1 Core and Protocol Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the tested Rust workspace, wire protocol, privacy-preserving line fingerprinting, line diff model, and provenance state engine shared by every later SVN AI component.

**Architecture:** A Cargo workspace contains two dependency-light libraries. `svn-ai-protocol` owns versioned serialized event contracts and identifiers; `svn-ai-core` owns deterministic byte-line splitting, keyed fingerprints, edit scripts, and provenance state transitions. The phase ends with golden JSON compatibility tests and an end-to-end library test that attributes AI lines, applies a later human rewrite, and calculates the surviving AI stock.

**Tech Stack:** Rust 2024 edition, Rust 1.85.1, Cargo, serde, serde_json, uuid, chrono, blake3, hex, thiserror, imara-diff, insta.

**Spec:** `docs/superpowers/specs/2026-09-08-svn-ai-attribution-system-design.md`

## Global Constraints

- Core components are implemented in Rust and compile on Windows and Linux.
- Line endings are excluded from fingerprints; all other line bytes, including indentation, remain significant.
- No API accepts or serializes Claude prompts, answers, transcripts, or whole source files.
- Wire contracts carry only repository-relative paths and reject absolute paths.
- Attribution decisions are deterministic; ambiguous matches are represented explicitly and never guessed as AI.
- Every production behavior is introduced by a failing test first.

---

## File Map

- `Cargo.toml`: workspace members and shared dependency versions.
- `rust-toolchain.toml`: pinned Rust 1.85.1 toolchain and rustfmt/clippy components.
- `.gitignore`: Rust build and local secret artifacts.
- `README.md`: project scope and phase status.
- `crates/svn-ai-protocol/Cargo.toml`: wire-contract crate manifest.
- `crates/svn-ai-protocol/src/lib.rs`: public protocol exports and schema version.
- `crates/svn-ai-protocol/src/events.rs`: AI attribution and SVN revision event types.
- `crates/svn-ai-protocol/src/identity.rs`: validated repository-relative path and digest types.
- `crates/svn-ai-protocol/tests/json_contract.rs`: JSON round-trip and rejection tests.
- `crates/svn-ai-protocol/tests/golden/ai-attribution-event.json`: stable example payload.
- `crates/svn-ai-core/Cargo.toml`: attribution engine crate manifest.
- `crates/svn-ai-core/src/lib.rs`: public core API.
- `crates/svn-ai-core/src/fingerprint.rs`: byte-line splitting and keyed fingerprints.
- `crates/svn-ai-core/src/edit.rs`: deterministic edit-script construction.
- `crates/svn-ai-core/src/provenance.rs`: line-origin state transitions and stock metrics.
- `crates/svn-ai-core/tests/fingerprint.rs`: newline, key, indentation, and context tests.
- `crates/svn-ai-core/tests/edit.rs`: insert, delete, replace, and repeated-line tests.
- `crates/svn-ai-core/tests/provenance.rs`: origin transition and stock tests.
- `crates/svn-ai-core/tests/end_to_end.rs`: cross-crate attribution lifecycle test.

### Task 1: Bootstrap the Rust workspace and versioned protocol

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.gitignore`
- Create: `README.md`
- Create: `crates/svn-ai-protocol/Cargo.toml`
- Create: `crates/svn-ai-protocol/src/lib.rs`
- Create: `crates/svn-ai-protocol/src/events.rs`
- Create: `crates/svn-ai-protocol/src/identity.rs`
- Test: `crates/svn-ai-protocol/tests/json_contract.rs`
- Test: `crates/svn-ai-protocol/tests/golden/ai-attribution-event.json`

**Interfaces:**
- Produces: `SCHEMA_VERSION: &str`, `RepoPath`, `Digest`, `AttributionEvent`, `AttributionHunk`, `RevisionEvent`, `RevisionFileChange`, `ChangeKind`, `ToolKind`, and `GeneratedLineState`.
- Consumes: no earlier task interfaces.

- [ ] **Step 1: Create only workspace metadata and the failing protocol test**

Create the workspace manifests, empty library modules, and this test before defining the event structs:

```rust
use svn_ai_protocol::{AttributionEvent, SCHEMA_VERSION};

#[test]
fn attribution_event_matches_the_golden_contract() {
    let json = include_str!("golden/ai-attribution-event.json");
    let event: AttributionEvent = serde_json::from_str(json).unwrap();

    assert_eq!(SCHEMA_VERSION, "svn-ai/1.0.0");
    assert_eq!(event.schema_version, SCHEMA_VERSION);
    assert_eq!(event.svn_username, "zhengjie");
    assert_eq!(event.hunks[0].new_line_digests.len(), 2);
    assert_eq!(serde_json::to_value(&event).unwrap(), serde_json::from_str::<serde_json::Value>(json).unwrap());
}
```

- [ ] **Step 2: Run the protocol test and verify RED**

Run: `cargo test -p svn-ai-protocol --test json_contract`

Expected: compilation fails because `AttributionEvent` and `SCHEMA_VERSION` are not defined.

- [ ] **Step 3: Implement the minimum versioned event contract**

Use string-backed UUIDs and timestamps on the wire so compatibility does not depend on serializer feature flags. Validate `RepoPath` during deserialization: reject drive prefixes, leading `/` or `\\`, `..` components, and NUL bytes. Define the following public shapes:

```rust
pub const SCHEMA_VERSION: &str = "svn-ai/1.0.0";

pub struct AttributionEvent {
    pub schema_version: String,
    pub event_id: String,
    pub device_id: String,
    pub client_version: String,
    pub svn_username: String,
    pub repository_uuid: String,
    pub repository_root_digest: Digest,
    pub base_revision: i64,
    pub session_id_digest: Digest,
    pub tool_use_id_digest: Digest,
    pub tool: ToolKind,
    pub occurred_at: String,
    pub model_id: Option<String>,
    pub path: RepoPath,
    pub hunks: Vec<AttributionHunk>,
}

pub struct AttributionHunk {
    pub old_start: u32,
    pub old_len: u32,
    pub new_start: u32,
    pub new_line_digests: Vec<Digest>,
    pub new_context_digests: Vec<Digest>,
}
```

Define matching revision-event types with repository UUID, revision, author, timestamp, path changes, copy-from information, exclusion status, and the same edit-hunk digest representation. All serialized enums use snake_case names.

- [ ] **Step 4: Add path-validation tests**

```rust
#[test]
fn repository_path_rejects_absolute_and_parent_paths() {
    for invalid in [r"C:\work\a.rs", "/srv/a.rs", "../secret.rs"] {
        assert!(svn_ai_protocol::RepoPath::try_from(invalid).is_err());
    }
}
```

- [ ] **Step 5: Run tests and verify GREEN**

Run: `cargo test -p svn-ai-protocol`

Expected: all protocol tests pass without warnings.

- [ ] **Step 6: Commit the protocol contract**

```bash
git add Cargo.toml rust-toolchain.toml .gitignore README.md crates/svn-ai-protocol
git commit -m "feat: define versioned SVN AI event protocol"
```

### Task 2: Implement privacy-preserving line fingerprints

**Files:**
- Create: `crates/svn-ai-core/Cargo.toml`
- Create: `crates/svn-ai-core/src/lib.rs`
- Create: `crates/svn-ai-core/src/fingerprint.rs`
- Test: `crates/svn-ai-core/tests/fingerprint.rs`

**Interfaces:**
- Consumes: `svn_ai_protocol::Digest`.
- Produces: `FingerprintKey::new([u8; 32])`, `split_lines(&[u8]) -> Vec<&[u8]>`, and `fingerprint_lines(&[u8], &FingerprintKey) -> FingerprintedFile`.

- [ ] **Step 1: Write newline and privacy tests**

```rust
use svn_ai_core::{fingerprint_lines, FingerprintKey};

#[test]
fn crlf_and_lf_produce_identical_line_digests() {
    let key = FingerprintKey::new([7; 32]);
    let lf = fingerprint_lines(b"alpha\nbeta\n", &key);
    let crlf = fingerprint_lines(b"alpha\r\nbeta\r\n", &key);
    assert_eq!(lf.line_digests, crlf.line_digests);
    assert_eq!(lf.context_digests, crlf.context_digests);
}

#[test]
fn indentation_and_key_changes_are_significant() {
    let key_a = FingerprintKey::new([1; 32]);
    let key_b = FingerprintKey::new([2; 32]);
    assert_ne!(
        fingerprint_lines(b"x", &key_a).line_digests,
        fingerprint_lines(b" x", &key_a).line_digests
    );
    assert_ne!(
        fingerprint_lines(b"x", &key_a).line_digests,
        fingerprint_lines(b"x", &key_b).line_digests
    );
}
```

- [ ] **Step 2: Run the tests and verify RED**

Run: `cargo test -p svn-ai-core --test fingerprint`

Expected: compilation fails because the fingerprint API does not exist.

- [ ] **Step 3: Implement byte-line splitting and keyed BLAKE3**

`split_lines` removes only `\n` and an immediately preceding `\r`. It preserves an empty final logical line only when the file contains content after the final delimiter. `fingerprint_lines` returns one keyed digest per logical line and one keyed context digest per line. A context digest hashes the previous digest or zero marker, current digest, and next digest or zero marker with explicit length framing.

- [ ] **Step 4: Add empty, non-UTF8, and repeated-line context tests**

```rust
#[test]
fn arbitrary_bytes_are_supported_without_source_text_serialization() {
    let key = FingerprintKey::new([9; 32]);
    let file = fingerprint_lines(&[0xff, b'\n', 0xfe], &key);
    assert_eq!(file.line_digests.len(), 2);
    assert!(serde_json::to_string(&file).unwrap().contains("ff") == false);
}

#[test]
fn context_disambiguates_identical_lines() {
    let key = FingerprintKey::new([3; 32]);
    let file = fingerprint_lines(b"before\nsame\nafter\nsame\nend", &key);
    assert_eq!(file.line_digests[1], file.line_digests[3]);
    assert_ne!(file.context_digests[1], file.context_digests[3]);
}
```

- [ ] **Step 5: Run all tests and verify GREEN**

Run: `cargo test -p svn-ai-core`

Expected: all fingerprint tests pass without warnings.

- [ ] **Step 6: Commit fingerprinting**

```bash
git add Cargo.toml crates/svn-ai-core
git commit -m "feat: add keyed line fingerprinting"
```

### Task 3: Build deterministic line edit scripts

**Files:**
- Create: `crates/svn-ai-core/src/edit.rs`
- Modify: `crates/svn-ai-core/src/lib.rs`
- Test: `crates/svn-ai-core/tests/edit.rs`

**Interfaces:**
- Consumes: `FingerprintKey`, `fingerprint_lines`, and `Digest`.
- Produces: `EditHunk`, `EditScript`, and `diff_files(before: &[u8], after: &[u8], key: &FingerprintKey) -> EditScript`.

- [ ] **Step 1: Write insert, delete, and replace tests**

```rust
use svn_ai_core::{diff_files, FingerprintKey};

#[test]
fn edit_script_reports_insert_delete_and_replace_ranges() {
    let key = FingerprintKey::new([4; 32]);

    let insert = diff_files(b"a\nc", b"a\nb\nc", &key);
    assert_eq!((insert.hunks[0].old_start, insert.hunks[0].old_len), (1, 0));
    assert_eq!((insert.hunks[0].new_start, insert.hunks[0].new_len()), (1, 1));

    let delete = diff_files(b"a\nb\nc", b"a\nc", &key);
    assert_eq!((delete.hunks[0].old_start, delete.hunks[0].old_len), (1, 1));
    assert_eq!(delete.hunks[0].new_len(), 0);

    let replace = diff_files(b"a\nb\nc", b"a\nx\nc", &key);
    assert_eq!((replace.hunks[0].old_len, replace.hunks[0].new_len()), (1, 1));
}
```

- [ ] **Step 2: Run the tests and verify RED**

Run: `cargo test -p svn-ai-core --test edit`

Expected: compilation fails because `diff_files` is not defined.

- [ ] **Step 3: Implement the imara-diff adapter**

Intern the line digests, run the Myers algorithm, and convert each non-equal range into a zero-based `EditHunk`. Store only new line and context digests in the hunk. Coalesce immediately adjacent non-equal ranges so the same input always emits the same JSON representation.

- [ ] **Step 4: Test repeated lines and deterministic output**

```rust
#[test]
fn repeated_lines_produce_stable_edit_scripts() {
    let key = FingerprintKey::new([5; 32]);
    let before = b"same\nleft\nsame\nright";
    let after = b"same\nchanged\nsame\nright";
    let first = diff_files(before, after, &key);
    let second = diff_files(before, after, &key);
    assert_eq!(first, second);
    assert_eq!(first.hunks.len(), 1);
}
```

- [ ] **Step 5: Run all core tests and verify GREEN**

Run: `cargo test -p svn-ai-core`

Expected: fingerprint and edit tests pass.

- [ ] **Step 6: Commit edit scripts**

```bash
git add crates/svn-ai-core
git commit -m "feat: compute deterministic line edit scripts"
```

### Task 4: Implement provenance transitions and stock metrics

**Files:**
- Create: `crates/svn-ai-core/src/provenance.rs`
- Modify: `crates/svn-ai-core/src/lib.rs`
- Test: `crates/svn-ai-core/tests/provenance.rs`

**Interfaces:**
- Consumes: `EditScript`, `EditHunk`, and `Digest`.
- Produces: `Origin`, `LineProvenance`, `FileProvenance`, `HunkAttribution`, `StockMetrics`, and `FileProvenance::apply(&EditScript, &[HunkAttribution])`.

- [ ] **Step 1: Write the failing origin-transition test**

```rust
use svn_ai_core::{FileProvenance, FingerprintKey, HunkAttribution, Origin, diff_files};

#[test]
fn ai_insert_then_human_rewrite_reduces_ai_stock() {
    let key = FingerprintKey::new([6; 32]);
    let mut state = FileProvenance::legacy(b"base", &key);

    let ai_edit = diff_files(b"base", b"base\nai one\nai two", &key);
    state.apply(&ai_edit, &[HunkAttribution::ai(0, "event-1")]).unwrap();
    assert_eq!(state.metrics().ai_lines, 2);
    assert_eq!(state.metrics().legacy_lines, 1);

    let human_edit = diff_files(b"base\nai one\nai two", b"base\nhuman\nai two", &key);
    state.apply(&human_edit, &[HunkAttribution::non_ai(0)]).unwrap();
    assert_eq!(state.metrics().ai_lines, 1);
    assert_eq!(state.metrics().non_ai_lines, 1);
}
```

- [ ] **Step 2: Run the test and verify RED**

Run: `cargo test -p svn-ai-core --test provenance`

Expected: compilation fails because provenance types do not exist.

- [ ] **Step 3: Implement provenance application**

Define the exact origin enum:

```rust
pub enum Origin {
    Ai { event_id: String },
    NonAi,
    Unattributed,
    Legacy,
}
```

Apply edit hunks from the end of the file toward the start so indices remain stable. Unchanged lines retain their origin. Deleted lines disappear. Every inserted line must receive exactly one origin assignment; missing, duplicate, or out-of-range assignments return a typed error instead of defaulting to AI.

- [ ] **Step 4: Add validation and copy tests**

```rust
#[test]
fn missing_hunk_attribution_is_an_error() {
    let key = FingerprintKey::new([8; 32]);
    let mut state = FileProvenance::legacy(b"a", &key);
    let edit = diff_files(b"a", b"a\nb", &key);
    assert!(state.apply(&edit, &[]).is_err());
}

#[test]
fn copied_state_preserves_origins_without_counting_new_ai() {
    let key = FingerprintKey::new([6; 32]);
    let mut source = FileProvenance::legacy(b"base", &key);
    let edit = diff_files(b"base", b"base\nai", &key);
    source.apply(&edit, &[HunkAttribution::ai(0, "event-2")]).unwrap();
    let copy = source.clone_for_svn_copy();
    assert_eq!(copy.metrics(), source.metrics());
    assert_eq!(copy.generated_ai_lines(), 0);
}
```

- [ ] **Step 5: Run all core tests and verify GREEN**

Run: `cargo test -p svn-ai-core`

Expected: all tests pass without warnings.

- [ ] **Step 6: Commit provenance transitions**

```bash
git add crates/svn-ai-core
git commit -m "feat: track line provenance and AI stock"
```

### Task 5: Prove cross-crate compatibility and document Phase 1

**Files:**
- Create: `crates/svn-ai-core/tests/end_to_end.rs`
- Modify: `README.md`

**Interfaces:**
- Consumes: the complete `svn-ai-protocol` and `svn-ai-core` public APIs.
- Produces: a stable foundation verified for use by the client, Hook Agent, and server phases.

- [ ] **Step 1: Write the failing end-to-end lifecycle test**

Build an `AttributionEvent` for two AI lines, serialize and deserialize it, compute an SVN-side edit script from the same content, assign matching hunks to the event, and assert the resulting `FileProvenance` contains two AI lines and one legacy line. Then replace one AI line as non-AI and assert one AI line survives.

- [ ] **Step 2: Run the end-to-end test and verify RED**

Run: `cargo test -p svn-ai-core --test end_to_end`

Expected: the test fails until protocol-to-core conversion helpers exist.

- [ ] **Step 3: Add the minimum conversion helpers**

Implement `From<&EditHunk> for svn_ai_protocol::AttributionHunk` and `TryFrom<&svn_ai_protocol::AttributionHunk> for EditHunk`. Hunk shape conversion rejects a protocol hunk whose line and context arrays have different lengths. Keep `HunkAttribution` separate because its event ID and hunk index come from the enclosing event and the matcher, not from the hunk payload itself.

- [ ] **Step 4: Run formatting, linting, and all tests**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: all commands succeed with no warnings.

- [ ] **Step 5: Update the README**

Document the system goal, privacy boundary, Rust toolchain, workspace crates, local test commands, and the fact that Phase 1 is a library foundation rather than a deployable client.

- [ ] **Step 6: Commit Phase 1 completion**

```bash
git add README.md crates Cargo.toml Cargo.lock rust-toolchain.toml
git commit -m "test: verify SVN AI attribution lifecycle"
```

## Phase 1 Completion Gate

Phase 1 is complete only when:

- `cargo fmt --all -- --check` passes;
- `cargo clippy --workspace --all-targets -- -D warnings` passes;
- `cargo test --workspace` passes;
- golden JSON contains no prompt, answer, transcript, or source text fields;
- Windows tests have run locally;
- the commit history contains independently reviewable protocol, fingerprint, edit, provenance, and lifecycle commits;
- the branch is pushed to `git@github.com:Jieszs/svn-ai.git` after the repository deploy key is authorized.
