use std::{error::Error, fs, path::PathBuf};

use chrono::Utc;
use clap::{Parser, Subcommand};
use svn_ai_core::{FingerprintKey, diff_files, match_attribution};
use svn_ai_protocol::{AttributionEvent, AttributionHunk, Digest, RepoPath, ToolKind};
use svn_ai_svn::SvnLook;
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(name = "svn-ai-validate")]
#[command(about = "Validate SVN AI attribution against a real repository revision")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Capture {
        #[arg(long)]
        before: PathBuf,
        #[arg(long)]
        after: PathBuf,
        #[arg(long)]
        repo_path: String,
        #[arg(long)]
        repository_uuid: String,
        #[arg(long)]
        svn_username: String,
        #[arg(long)]
        base_revision: i64,
        #[arg(long)]
        key_hex: String,
        #[arg(long)]
        out: PathBuf,
    },
    Verify {
        #[arg(long)]
        svnlook: PathBuf,
        #[arg(long)]
        repository: PathBuf,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        attribution: PathBuf,
        #[arg(long)]
        key_hex: String,
        #[arg(long)]
        json: bool,
    },
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("svn-ai-validate: {error}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    match cli.command {
        Commands::Capture {
            before,
            after,
            repo_path,
            repository_uuid,
            svn_username,
            base_revision,
            key_hex,
            out,
        } => {
            let key_bytes = parse_key(&key_hex)?;
            let key = FingerprintKey::new(key_bytes);
            let before_content = fs::read(before)?;
            let after_content = fs::read(after)?;
            let script = diff_files(&before_content, &after_content, &key);
            let event_id = Uuid::now_v7().to_string();
            let event = AttributionEvent {
                schema_version: "1.0.0".to_owned(),
                event_id,
                device_id: "validation-device".to_owned(),
                client_version: env!("CARGO_PKG_VERSION").to_owned(),
                svn_username,
                repository_uuid: repository_uuid.clone(),
                repository_root_digest: keyed_digest(
                    &key_bytes,
                    b"svn-ai/validation/repository/v1\0",
                    repository_uuid.as_bytes(),
                ),
                base_revision,
                session_id_digest: keyed_digest(
                    &key_bytes,
                    b"svn-ai/validation/session/v1\0",
                    b"validation-session",
                ),
                tool_use_id_digest: keyed_digest(
                    &key_bytes,
                    b"svn-ai/validation/tool/v1\0",
                    b"validation-tool",
                ),
                tool: ToolKind::Edit,
                occurred_at: Utc::now().to_rfc3339(),
                model_id: Some("claude-validation".to_owned()),
                path: RepoPath::try_from(repo_path)?,
                hunks: script.hunks.iter().map(AttributionHunk::from).collect(),
            };
            fs::write(out, serde_json::to_vec_pretty(&event)?)?;
        }
        Commands::Verify {
            svnlook,
            repository,
            revision,
            attribution,
            key_hex,
            json,
        } => {
            let key = FingerprintKey::new(parse_key(&key_hex)?);
            let event: AttributionEvent = serde_json::from_slice(&fs::read(attribution)?)?;
            let revision_event =
                SvnLook::new(svnlook).read_revision(&repository, revision, &key)?;
            let metrics = match_attribution(&revision_event, &[event]);
            if json {
                println!("{}", serde_json::to_string_pretty(&metrics)?);
            } else {
                println!(
                    "SVN additions: {}, AI additions: {}, non-AI additions: {}, ambiguous additions: {}",
                    metrics.svn_additions,
                    metrics.ai_additions,
                    metrics.non_ai_additions,
                    metrics.ambiguous_additions,
                );
            }
        }
    }
    Ok(())
}

fn parse_key(value: &str) -> Result<[u8; 32], Box<dyn Error>> {
    let mut bytes = [0_u8; 32];
    hex::decode_to_slice(value, &mut bytes)
        .map_err(|_| "--key-hex must contain exactly 64 hexadecimal characters")?;
    Ok(bytes)
}

fn keyed_digest(key: &[u8; 32], domain: &[u8], value: &[u8]) -> Digest {
    let mut hasher = blake3::Hasher::new_keyed(key);
    hasher.update(domain);
    hasher.update(value);
    Digest::new(*hasher.finalize().as_bytes())
}
