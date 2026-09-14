# svn-ai

`svn-ai` is an offline-first attribution and statistics system for code written with Claude Code and committed to Subversion. It is designed for Windows and Linux developer clients, VisualSVN Server hooks, and a centrally deployed Linux service.

## Privacy boundary

The wire protocol contains keyed line fingerprints and repository-relative paths. It does not contain Claude prompts, answers, transcripts, or complete source files.

## Current phase

The repository currently provides the shared foundation and the first usable Claude Code client:

- `svn-ai-protocol`: versioned JSON event contracts and validated identifiers;
- `svn-ai-core`: byte-safe line fingerprinting, deterministic edit scripts, line-provenance transitions, and one-to-one AI/SVN line matching;
- `svn-ai-svn`: a reusable `svnlook` adapter limited to commands available in Subversion 1.7;
- `svn-ai`: a Windows/Linux client that captures Claude Code `Edit` and `Write` operations through hooks, keeps short-lived source snapshots in local SQLite, and persists only fingerprint attribution events;
- `svn-ai-validate`: a diagnostic CLI for recording fingerprint-only attribution and checking it against a real repository revision.

The VisualSVN Hook Agent, automatic event upload, and centralized Linux statistics service are not implemented yet. The current `stats` command is a local validation path and requires filesystem access to the SVN repository.

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

## Build and configure the Claude Code client

Build the release executable on Windows or Linux:

```bash
cargo build -p svn-ai --release
```

Configure one client identity per developer. The fingerprint key is a shared 32-byte secret encoded as 64 hexadecimal characters; distribute it through the organization's approved secret-delivery process.

```powershell
svn-ai configure `
  --svn-username <personal-svn-account> `
  --fingerprint-key <64-hex-character-key> `
  --svn "C:\Program Files\TortoiseSVN\bin\svn.exe"
```

On Linux, use the same command with the local `svn` executable, for example `--svn /usr/bin/svn`. Add the global `--home <path>` option before the subcommand when the data directory must be placed somewhere other than the platform default.

Install the three non-blocking Claude Code hooks into the current user's settings:

```powershell
svn-ai install-hooks
```

The installer preserves unrelated settings and hooks, and creates `settings.json.svn-ai.bak` before changing an existing settings file. It registers `PreToolUse`, `PostToolUse`, and `PostToolUseFailure` for the `Edit|Write` matcher. Restart Claude Code after installation.

Check local capture state and inspect fingerprint-only events:

```powershell
svn-ai status --json
svn-ai events --json
```

After committing through SVN, an administrator or diagnostic environment with filesystem access to the repository can calculate one revision locally:

```powershell
svn-ai stats `
  --repository <svn-repository-filesystem-path> `
  --revision <revision-number> `
  --svnlook <path-to-svnlook> `
  --json
```

Claude Code `Bash` filesystem changes are not captured in this increment. Normal centralized use also still requires the later event-upload client, VisualSVN Hook Agent, and Linux statistics service; developers should not be given direct repository filesystem access just to run `stats`.

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

## Run the automatic Claude-hook SVN round trip

On Windows with Docker Desktop running:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-claude-svn-e2e.ps1
```

This test installs the production hooks into an isolated Claude settings file, sends the official hook JSON lifecycle around a ten-line edit, verifies that the persisted event does not contain source text, absolute paths, the raw session identifier, or a transcript, manually rewrites two lines, commits through real SVN, and checks the same `10 / 8 / 2 / 0` statistics.

## Design documents

- [System design](docs/superpowers/specs/2026-09-08-svn-ai-attribution-system-design.md)
- [Phase 1 implementation plan](docs/superpowers/plans/2026-09-08-phase-1-core-protocol.md)
- [Claude Code client implementation plan](docs/superpowers/plans/2026-09-14-claude-code-client.md)
