use std::{
    error::Error,
    fs::OpenOptions,
    io::{Read, Write},
    path::PathBuf,
};

use chrono::Utc;
use clap::{Parser, Subcommand};
use serde::Serialize;
use svn_ai::{
    ClientConfig, EventStore, FingerprintSecret, HookInput, HookProcessor, install_claude_hooks,
};
use svn_ai_core::{FingerprintKey, match_attribution};
use svn_ai_svn::{SvnClient, SvnLook};
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(name = "svn-ai", version, about = "Claude Code attribution for SVN")]
struct Cli {
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Configure {
        #[arg(long)]
        device_id: Option<String>,
        #[arg(long)]
        svn_username: String,
        #[arg(long)]
        fingerprint_key: String,
        #[arg(long, default_value = "svn")]
        svn: PathBuf,
    },
    InstallHooks {
        #[arg(long)]
        settings: Option<PathBuf>,
        #[arg(long)]
        executable: Option<PathBuf>,
    },
    Hook,
    Status {
        #[arg(long)]
        json: bool,
    },
    Events {
        #[arg(long)]
        json: bool,
    },
    Stats {
        #[arg(long)]
        repository: PathBuf,
        #[arg(long)]
        revision: i64,
        #[arg(long, default_value = "svnlook")]
        svnlook: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let home = match resolve_home(cli.home) {
        Ok(home) => home,
        Err(error) => {
            eprintln!("svn-ai: {error}");
            std::process::exit(1);
        }
    };
    if matches!(cli.command, Commands::Hook) {
        if let Err(error) = run_hook(&home) {
            let _ = record_diagnostic(&home, &error.to_string());
            eprintln!("svn-ai hook: capture skipped: {error}");
        }
        return;
    }
    if let Err(error) = run(cli.command, &home) {
        eprintln!("svn-ai: {error}");
        std::process::exit(1);
    }
}

fn run(command: Commands, home: &std::path::Path) -> Result<(), Box<dyn Error>> {
    match command {
        Commands::Configure {
            device_id,
            svn_username,
            fingerprint_key,
            svn,
        } => {
            ClientConfig {
                device_id: device_id.unwrap_or_else(|| Uuid::now_v7().to_string()),
                svn_username,
                fingerprint_secret: FingerprintSecret::parse(&fingerprint_key)?,
                svn_executable: svn,
            }
            .save(home)?;
            println!("configured");
        }
        Commands::InstallHooks {
            settings,
            executable,
        } => {
            let settings = settings.unwrap_or(default_claude_settings()?);
            let executable = executable.unwrap_or(std::env::current_exe()?);
            let result = install_claude_hooks(&settings, &executable, home)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Status { json } => {
            let config = ClientConfig::load(home)?;
            let status = EventStore::open(home)?.status()?;
            let output = ClientStatus {
                configured: true,
                device_id: config.device_id,
                svn_username: config.svn_username,
                pending_transactions: status.pending_transactions,
                attribution_events: status.attribution_events,
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!(
                    "configured: yes, pending: {}, events: {}",
                    output.pending_transactions, output.attribution_events
                );
            }
        }
        Commands::Events { json } => {
            let events = EventStore::open(home)?.list_events()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&events)?);
            } else {
                println!("{} attribution events", events.len());
            }
        }
        Commands::Stats {
            repository,
            revision,
            svnlook,
            json,
        } => {
            let config = ClientConfig::load(home)?;
            let events = EventStore::open(home)?.list_events()?;
            let key = FingerprintKey::new(*config.fingerprint_secret.as_bytes());
            let revision_event =
                SvnLook::new(svnlook).read_revision(&repository, revision, &key)?;
            let metrics = match_attribution(&revision_event, &events);
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
        Commands::Hook => unreachable!("hook is handled as a non-blocking command"),
    }
    Ok(())
}

fn run_hook(home: &std::path::Path) -> Result<(), Box<dyn Error>> {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input)?;
    let hook_input: HookInput =
        serde_json::from_slice(&input).map_err(|error| format!("invalid hook JSON: {error}"))?;
    let config = ClientConfig::load(home)?;
    let store = EventStore::open(home)?;
    let svn = SvnClient::new(config.svn_executable.clone());
    HookProcessor::new(&config, &store, &svn).process(hook_input)?;
    Ok(())
}

fn resolve_home(explicit: Option<PathBuf>) -> Result<PathBuf, Box<dyn Error>> {
    if let Some(home) = explicit {
        return Ok(home);
    }
    if let Some(home) = std::env::var_os("SVN_AI_HOME") {
        return Ok(PathBuf::from(home));
    }
    #[cfg(windows)]
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(local_app_data).join("svn-ai"));
    }
    #[cfg(not(windows))]
    {
        if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(data_home).join("svn-ai"));
        }
        if let Some(user_home) = std::env::var_os("HOME") {
            return Ok(PathBuf::from(user_home)
                .join(".local")
                .join("share")
                .join("svn-ai"));
        }
    }
    Err("cannot determine svn-ai home; pass --home".into())
}

fn default_claude_settings() -> Result<PathBuf, Box<dyn Error>> {
    if let Some(config_dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Ok(PathBuf::from(config_dir).join("settings.json"));
    }
    let user_home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .ok_or("cannot determine Claude settings path; pass --settings")?;
    Ok(PathBuf::from(user_home)
        .join(".claude")
        .join("settings.json"))
}

fn record_diagnostic(home: &std::path::Path, message: &str) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(home)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(home.join("diagnostics.log"))?;
    writeln!(file, "{} {message}", Utc::now().to_rfc3339())
}

#[derive(Debug, Serialize)]
struct ClientStatus {
    configured: bool,
    device_id: String,
    svn_username: String,
    pending_transactions: u64,
    attribution_events: u64,
}
