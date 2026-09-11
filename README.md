# svn-ai

`svn-ai` is an offline-first attribution and statistics system for code written with Claude Code and committed to Subversion. It is designed for Windows and Linux developer clients, VisualSVN Server hooks, and a centrally deployed Linux service.

## Privacy boundary

The wire protocol contains keyed line fingerprints and repository-relative paths. It does not contain Claude prompts, answers, transcripts, or complete source files.

## Current phase

The repository currently provides the shared foundation used by all later binaries:

- `svn-ai-protocol`: versioned JSON event contracts and validated identifiers;
- `svn-ai-core`: byte-safe line fingerprinting, deterministic edit scripts, line-provenance transitions, and one-to-one AI/SVN line matching;
- `svn-ai-svn`: a reusable `svnlook` adapter limited to commands available in Subversion 1.7;
- `svn-ai-validate`: a diagnostic CLI for recording fingerprint-only attribution and checking it against a real repository revision.

This is not yet the deployable Claude Code client, VisualSVN Hook Agent, or statistics server. The validation executable exists to prove the critical SVN integration path before those components are built.

## Toolchain

- Rust 1.85.1
- Rust 2024 edition
- Cargo with Clippy and Rustfmt

## Verify locally

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Run the real SVN round trip

On Windows with Docker Desktop running:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-svn-e2e.ps1
```

The test creates an isolated repository, commits a one-line baseline, records ten AI-added lines as keyed fingerprints, manually rewrites two lines, commits revision 2 through `svn`, then reads that revision through `svnlook`. The expected result is:

```json
{
  "svn_additions": 10,
  "ai_additions": 8,
  "non_ai_additions": 2,
  "ambiguous_additions": 0
}
```

The container currently exercises Subversion 1.14.2 while the adapter deliberately uses only the `author`, `date`, `uuid`, `changed --copy-info`, and `cat` commands documented for Subversion 1.7. A final smoke test using the installed `svnlook.exe` on the bank's VisualSVN Server 2.5.2 host remains required before production rollout.

## Design documents

- [System design](docs/superpowers/specs/2026-09-08-svn-ai-attribution-system-design.md)
- [Phase 1 implementation plan](docs/superpowers/plans/2026-09-08-phase-1-core-protocol.md)
