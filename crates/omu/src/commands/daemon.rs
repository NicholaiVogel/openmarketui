use super::{audit_event, require_yes};
use crate::cli::{Cli, DaemonCommand, DaemonStartArgs, DaemonStopArgs, DaemonSubcommand};
use crate::client::DaemonClient;
use crate::error::CliError;
use crate::output::OutputContext;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DaemonState {
    pid: u32,
    started_at: String,
    command: Vec<String>,
    config_path: String,
    log_path: String,
}

pub(super) async fn handle(
    cli: &Cli,
    context: &OutputContext,
    client: &DaemonClient,
    command: &DaemonCommand,
) -> Result<Value, CliError> {
    match &command.command {
        DaemonSubcommand::Status => status(cli, client).await,
        DaemonSubcommand::Start(args) => start(cli, context, client, args).await,
        DaemonSubcommand::Stop(args) => stop(cli, client, args).await,
        DaemonSubcommand::Logs(args) => logs(cli, args.lines),
    }
}

pub(super) async fn probe(client: &DaemonClient) -> Result<Value, CliError> {
    let status = client.get_value("/api/status").await?;
    let session = client.get_value("/api/session/status").await?;
    Ok(json!({ "daemon": status, "session": session }))
}

async fn status(cli: &Cli, client: &DaemonClient) -> Result<Value, CliError> {
    let local = local_status(cli)?;
    match probe(client).await {
        Ok(remote) => Ok(json!({
            "reachable": true,
            "remote": remote,
            "local": local,
        })),
        Err(err) => Ok(json!({
            "reachable": false,
            "remote_error": {
                "code": err.code(),
                "message": err.to_string(),
                "hint": err.hint(),
            },
            "local": local,
        })),
    }
}

async fn start(
    cli: &Cli,
    context: &OutputContext,
    client: &DaemonClient,
    args: &DaemonStartArgs,
) -> Result<Value, CliError> {
    let kalshi_bin = resolve_kalshi_bin(args.kalshi_bin.as_deref());
    let config_path = args
        .config
        .clone()
        .or_else(|| context.kalshi_config.as_ref().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("config.toml"));
    let log_path = log_path(cli)?;

    if cli.dry_run {
        return Ok(json!({
            "dry_run": true,
            "would": {
                "action": "start daemon",
                "command": command_display(&kalshi_bin, &config_path),
                "foreground": args.foreground,
                "timeout_secs": args.timeout_secs,
            },
            "local": {
                "state_path": state_path(cli)?.display().to_string(),
                "log_path": log_path.display().to_string(),
            }
        }));
    }

    if let Ok(remote) = probe(client).await {
        return Ok(json!({
            "started": false,
            "already_running": true,
            "reachable": true,
            "remote": remote,
            "local": local_status(cli)?,
        }));
    }

    if let Some(state) = read_state(cli)? {
        if pid_alive(state.pid) {
            return Ok(json!({
                "started": false,
                "already_running": true,
                "reachable": false,
                "local": state,
                "message": "local daemon pid is running, but the daemon API is not reachable",
            }));
        }
    }

    fs::create_dir_all(state_dir(cli)?)?;
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)?;
    }

    if args.foreground {
        let status = Command::new(&kalshi_bin)
            .arg("paper")
            .arg("--config")
            .arg(&config_path)
            .status()?;
        return Ok(json!({
            "foreground": true,
            "command": command_display(&kalshi_bin, &config_path),
            "exit_status": status.code(),
            "success": status.success(),
        }));
    }

    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let log_for_stderr = log.try_clone()?;

    let mut child = Command::new(&kalshi_bin)
        .arg("paper")
        .arg("--config")
        .arg(&config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_for_stderr))
        .spawn()?;

    let pid = child.id();
    let state = DaemonState {
        pid,
        started_at: Utc::now().to_rfc3339(),
        command: command_display(&kalshi_bin, &config_path),
        config_path: config_path.display().to_string(),
        log_path: log_path.display().to_string(),
    };
    write_state(cli, &state)?;

    let deadline = Instant::now() + Duration::from_secs(args.timeout_secs);
    while Instant::now() < deadline {
        if let Ok(remote) = probe(client).await {
            let result = json!({
                "started": true,
                "reachable": true,
                "remote": remote,
                "local": state.clone(),
            });
            let audit = audit_event(
                client,
                cli,
                "daemon.start",
                json!({
                    "command": state.command.clone(),
                    "config_path": state.config_path.clone(),
                    "foreground": false,
                    "timeout_secs": args.timeout_secs,
                }),
                result.clone(),
            )
            .await;
            return Ok(json!({
                "started": true,
                "reachable": true,
                "remote": result["remote"].clone(),
                "local": result["local"].clone(),
                "audit": audit,
            }));
        }

        if let Some(exit_status) = child.try_wait()? {
            return Err(CliError::Process {
                message: format!(
                    "daemon exited before becoming reachable with status {}; log: {}",
                    exit_status,
                    log_path.display()
                ),
            });
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    Ok(json!({
        "started": true,
        "reachable": false,
        "local": state,
        "warning": format!(
            "daemon did not become reachable at {} within {}s",
            context.daemon_url,
            args.timeout_secs
        ),
    }))
}

async fn stop(cli: &Cli, client: &DaemonClient, args: &DaemonStopArgs) -> Result<Value, CliError> {
    let state = read_state(cli)?;
    if cli.dry_run {
        return Ok(json!({
            "stopped": false,
            "dry_run": true,
            "would": {
                "action": "stop daemon",
                "method": "POST",
                "path": "/api/daemon/shutdown",
                "force": args.force,
                "timeout_secs": args.timeout_secs,
            },
            "local": state,
        }));
    }

    require_yes(cli, "stopping the local daemon")?;

    let audit = audit_event(
        client,
        cli,
        "daemon.stop",
        json!({
            "force": args.force,
            "timeout_secs": args.timeout_secs,
            "local": state.clone(),
        }),
        json!({
            "requested": true,
        }),
    )
    .await;

    let api_shutdown = client.post_empty("/api/daemon/shutdown").await;
    let mut signal_sent = false;

    if let Some(ref state) = state {
        if pid_alive(state.pid) {
            signal_sent = true;
            wait_until_dead(state.pid, Duration::from_secs(args.timeout_secs));
            if pid_alive(state.pid) {
                kill_pid(state.pid, "-TERM")?;
                wait_until_dead(state.pid, Duration::from_secs(3));
            }
            if args.force && pid_alive(state.pid) {
                kill_pid(state.pid, "-KILL")?;
                wait_until_dead(state.pid, Duration::from_secs(2));
            }
        }
    }

    let local_after = local_status(cli)?;
    let stopped = local_after
        .get("pid_running")
        .and_then(Value::as_bool)
        .map(|running| !running)
        .unwrap_or_else(|| api_shutdown.is_ok());

    if stopped {
        let _ = fs::remove_file(state_path(cli)?);
    }

    if !stopped && api_shutdown.is_err() && !signal_sent {
        return Err(CliError::Process {
            message: "no reachable daemon API and no running local daemon pid found".to_string(),
        });
    }

    Ok(json!({
        "stopped": stopped,
        "api_shutdown": api_shutdown.is_ok(),
        "signal_sent": signal_sent,
        "local": local_after,
        "audit": audit,
    }))
}

fn logs(cli: &Cli, lines: usize) -> Result<Value, CliError> {
    let path = read_state(cli)?
        .map(|state| PathBuf::from(state.log_path))
        .unwrap_or(log_path(cli)?);

    if !path.exists() {
        return Err(CliError::NotFound {
            resource: "daemon log".to_string(),
            id: path.display().to_string(),
        });
    }

    let content = fs::read_to_string(&path)?;
    let lines = tail_lines(&content, lines);
    Ok(json!({
        "path": path.display().to_string(),
        "lines": lines,
    }))
}

fn local_status(cli: &Cli) -> Result<Value, CliError> {
    let state = read_state(cli)?;
    let pid_running = state.as_ref().map(|state| pid_alive(state.pid));
    Ok(json!({
        "state_path": state_path(cli)?.display().to_string(),
        "log_path": log_path(cli)?.display().to_string(),
        "pid_running": pid_running,
        "state": state,
    }))
}

fn read_state(cli: &Cli) -> Result<Option<DaemonState>, CliError> {
    let path = state_path(cli)?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&content)?))
}

fn write_state(cli: &Cli, state: &DaemonState) -> Result<(), CliError> {
    let path = state_path(cli)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(state)?)?;
    Ok(())
}

fn state_path(cli: &Cli) -> Result<PathBuf, CliError> {
    Ok(state_dir(cli)?.join(format!("daemon-{}.json", sanitize(cli.profile_name()))))
}

fn log_path(cli: &Cli) -> Result<PathBuf, CliError> {
    Ok(state_dir(cli)?.join(format!("daemon-{}.log", sanitize(cli.profile_name()))))
}

fn state_dir(cli: &Cli) -> Result<PathBuf, CliError> {
    if let Some(config_dir) = &cli.config_dir {
        return Ok(config_dir.join("state"));
    }
    if let Ok(dir) = std::env::var("OMU_STATE_DIR") {
        return Ok(PathBuf::from(dir));
    }
    if let Ok(dir) = std::env::var("XDG_STATE_HOME") {
        return Ok(PathBuf::from(dir).join("openmarketui"));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Ok(PathBuf::from(home).join(".local/state/openmarketui"));
    }
    Ok(std::env::current_dir()?.join(".omu-state"))
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn resolve_kalshi_bin(explicit: Option<&Path>) -> PathBuf {
    if let Some(path) = explicit {
        return path.to_path_buf();
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            let candidate = dir.join(format!("kalshi{}", std::env::consts::EXE_SUFFIX));
            if candidate.exists() {
                return candidate;
            }
        }
    }

    for candidate in ["target/debug/kalshi", "target/release/kalshi"] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return path;
        }
    }

    PathBuf::from("kalshi")
}

fn command_display(kalshi_bin: &Path, config_path: &Path) -> Vec<String> {
    vec![
        kalshi_bin.display().to_string(),
        "paper".to_string(),
        "--config".to_string(),
        config_path.display().to_string(),
    ]
}

fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn kill_pid(pid: u32, signal: &str) -> Result<(), CliError> {
    let status = Command::new("kill")
        .arg(signal)
        .arg(pid.to_string())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(CliError::Process {
            message: format!("failed to send {signal} to pid {pid}"),
        })
    }
}

fn wait_until_dead(pid: u32, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !pid_alive(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn tail_lines(content: &str, count: usize) -> Vec<String> {
    if count == 0 {
        return Vec::new();
    }
    let mut lines: Vec<_> = content
        .lines()
        .rev()
        .take(count)
        .map(ToString::to_string)
        .collect();
    lines.reverse();
    lines
}
