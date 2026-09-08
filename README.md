# svn-ai

`svn-ai` is an offline-first attribution and statistics system for code written with Claude Code and committed to Subversion. It is designed for Windows and Linux developer clients, VisualSVN Server hooks, and a centrally deployed Linux service.

## Privacy boundary

The wire protocol contains keyed line fingerprints and repository-relative paths. It does not contain Claude prompts, answers, transcripts, or complete source files.

## Current phase

Phase 1 provides the shared foundation used by all later binaries:

- `svn-ai-protocol`: versioned JSON event contracts and validated identifiers;
- `svn-ai-core`: byte-safe line fingerprinting, deterministic edit scripts, and line-provenance transitions.

This phase is a tested library foundation, not yet a deployable client or server.

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

## Design documents

- [System design](docs/superpowers/specs/2026-09-08-svn-ai-attribution-system-design.md)
- [Phase 1 implementation plan](docs/superpowers/plans/2026-09-08-phase-1-core-protocol.md)
