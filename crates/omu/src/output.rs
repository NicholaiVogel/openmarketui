use crate::cli::OutputFormat;
use crate::error::CliError;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct OutputContext {
    pub profile: String,
    pub profile_exists: bool,
    pub daemon_url: String,
    pub config_path: String,
    pub kalshi_config: Option<String>,
    pub dry_run: bool,
    pub trace_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct OkEnvelope {
    ok: bool,
    data: Value,
    meta: OutputContext,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope<'a> {
    ok: bool,
    error: ErrorBody,
    meta: &'a OutputContext,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
    hint: Option<&'static str>,
}

pub fn print_ok(format: OutputFormat, context: &OutputContext, data: Value) -> anyhow::Result<()> {
    match format {
        OutputFormat::Json => {
            let envelope = OkEnvelope {
                ok: true,
                data,
                meta: context.clone(),
            };
            println!("{}", serde_json::to_string_pretty(&envelope)?);
        }
        OutputFormat::Human => {
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
    }
    Ok(())
}

pub fn print_error(
    format: OutputFormat,
    context: &OutputContext,
    err: &CliError,
) -> anyhow::Result<()> {
    match format {
        OutputFormat::Json => {
            let envelope = ErrorEnvelope {
                ok: false,
                error: ErrorBody {
                    code: err.code(),
                    message: err.to_string(),
                    hint: err.hint(),
                },
                meta: context,
            };
            eprintln!("{}", serde_json::to_string_pretty(&envelope)?);
        }
        OutputFormat::Human => {
            eprintln!("error: {err}");
            if let Some(hint) = err.hint() {
                eprintln!("hint: {hint}");
            }
        }
    }
    Ok(())
}
