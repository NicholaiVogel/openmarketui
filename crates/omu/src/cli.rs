use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::settings::{DEFAULT_DAEMON_URL, DEFAULT_PROFILE};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum OutputFormat {
    Json,
    Human,
}

#[derive(Debug, Parser)]
#[command(name = "omu")]
#[command(about = "OpenMarketUI daemon-attached CLI")]
pub(crate) struct Cli {
    #[arg(long, value_enum, default_value_t = OutputFormat::Json, global = true)]
    pub(crate) format: OutputFormat,

    #[arg(long, global = true)]
    pub(crate) profile: Option<String>,

    #[arg(long, global = true)]
    pub(crate) config_dir: Option<PathBuf>,

    #[arg(long, env = "OMU_DAEMON_URL", global = true)]
    pub(crate) daemon_url: Option<String>,

    #[arg(long, global = true)]
    pub(crate) yes: bool,

    #[arg(long, global = true)]
    pub(crate) dry_run: bool,

    #[arg(skip = true)]
    pub(crate) policy_require_yes: bool,

    #[arg(skip = true)]
    pub(crate) policy_allow_live: bool,

    #[arg(skip = None)]
    pub(crate) policy_max_position_usd: Option<f64>,

    #[arg(skip = None)]
    pub(crate) policy_max_bankroll_usd: Option<f64>,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

impl Cli {
    pub(crate) fn profile_name(&self) -> &str {
        self.profile.as_deref().unwrap_or(DEFAULT_PROFILE)
    }

    pub(crate) fn daemon_url(&self) -> String {
        self.daemon_url
            .clone()
            .or_else(|| std::env::var("OPENMARKETUI_DAEMON_URL").ok())
            .unwrap_or_else(|| DEFAULT_DAEMON_URL.to_string())
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    Daemon(DaemonCommand),
    Overview,
    Portfolio(PortfolioCommand),
    Positions(PositionsCommand),
    Trades(TradesCommand),
    Markets(MarketsCommand),
    Pipeline(PipelineCommand),
    Ingest(IngestCommand),
    Backtest(BacktestCommand),
    Sessions(SessionsCommand),
    Audit(AuditCommand),
    Config(ConfigCommand),
    Profiles(ProfilesCommand),
}

#[derive(Debug, Args)]
pub(crate) struct DaemonCommand {
    #[command(subcommand)]
    pub(crate) command: DaemonSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum DaemonSubcommand {
    Status,
    Start(DaemonStartArgs),
    Stop(DaemonStopArgs),
    Logs(DaemonLogsArgs),
}

#[derive(Debug, Args)]
pub(crate) struct DaemonStartArgs {
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,

    #[arg(long)]
    pub(crate) kalshi_bin: Option<PathBuf>,

    #[arg(long)]
    pub(crate) foreground: bool,

    #[arg(long, default_value_t = 15)]
    pub(crate) timeout_secs: u64,
}

#[derive(Debug, Args)]
pub(crate) struct DaemonStopArgs {
    #[arg(long)]
    pub(crate) force: bool,

    #[arg(long, default_value_t = 10)]
    pub(crate) timeout_secs: u64,
}

#[derive(Debug, Args)]
pub(crate) struct DaemonLogsArgs {
    #[arg(short = 'n', long, default_value_t = 80)]
    pub(crate) lines: usize,
}

#[derive(Debug, Args)]
pub(crate) struct PortfolioCommand {
    #[command(subcommand)]
    pub(crate) command: PortfolioSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum PortfolioSubcommand {
    Summary,
    History,
    EquityCurve,
}

#[derive(Debug, Args)]
pub(crate) struct PositionsCommand {
    #[command(subcommand)]
    pub(crate) command: PositionsSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum PositionsSubcommand {
    List,
    Show { ticker: String },
    Close { ticker: String },
}

#[derive(Debug, Args)]
pub(crate) struct TradesCommand {
    #[command(subcommand)]
    pub(crate) command: TradesSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TradesSubcommand {
    List,
}

#[derive(Debug, Args)]
pub(crate) struct MarketsCommand {
    #[command(subcommand)]
    pub(crate) command: MarketsSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum MarketsSubcommand {
    List {
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    Show {
        ticker: String,
        #[arg(long, default_value_t = 500)]
        limit: usize,
    },
    Search {
        query: String,
        #[arg(long, default_value_t = 500)]
        limit: usize,
    },
}

#[derive(Debug, Args)]
pub(crate) struct PipelineCommand {
    #[command(subcommand)]
    pub(crate) command: PipelineSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum PipelineSubcommand {
    Status,
    Scorers(ScorersCommand),
    Filters(FiltersCommand),
    Decisions(DecisionsCommand),
}

#[derive(Debug, Args)]
pub(crate) struct ScorersCommand {
    #[command(subcommand)]
    pub(crate) command: ScorersSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ScorersSubcommand {
    List,
    Show { name: String },
    Enable { name: String },
    Disable { name: String },
}

#[derive(Debug, Args)]
pub(crate) struct FiltersCommand {
    #[command(subcommand)]
    pub(crate) command: FiltersSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum FiltersSubcommand {
    List,
    Show { name: String },
}

#[derive(Debug, Args)]
pub(crate) struct DecisionsCommand {
    #[command(subcommand)]
    pub(crate) command: DecisionsSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum DecisionsSubcommand {
    List {
        #[arg(long, default_value_t = 100)]
        limit: u32,
    },
    Show {
        id: i64,
    },
    Ticker {
        ticker: String,
        #[arg(long, default_value_t = 100)]
        limit: u32,
    },
}

#[derive(Debug, Args)]
pub(crate) struct IngestCommand {
    #[command(subcommand)]
    pub(crate) command: IngestSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum IngestSubcommand {
    Status,
    Fetch(DataFetchArgs),
    Cancel,
}

#[derive(Debug, Args)]
pub(crate) struct DataFetchArgs {
    #[arg(long)]
    pub(crate) start: String,
    #[arg(long)]
    pub(crate) end: String,
    #[arg(long, default_value_t = 100_000)]
    pub(crate) trades_per_day: usize,
    #[arg(long, default_value_t = true)]
    pub(crate) fetch_markets: bool,
    #[arg(long, default_value_t = true)]
    pub(crate) fetch_trades: bool,
}

#[derive(Debug, Args)]
pub(crate) struct BacktestCommand {
    #[command(subcommand)]
    pub(crate) command: BacktestSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum BacktestSubcommand {
    Run(BacktestRunArgs),
    Status,
    Summary,
    List {
        #[arg(long, default_value_t = 25)]
        limit: u32,
    },
    Show {
        id: String,
    },
    Compare(BacktestCompareArgs),
    Stop,
}

#[derive(Debug, Args)]
pub(crate) struct BacktestCompareArgs {
    pub(crate) baseline: String,
    pub(crate) challenger: String,
}

#[derive(Debug, Args)]
pub(crate) struct BacktestRunArgs {
    #[arg(long)]
    pub(crate) start: String,
    #[arg(long)]
    pub(crate) end: String,
    #[arg(long)]
    pub(crate) capital: Option<f64>,
    #[arg(long)]
    pub(crate) max_positions: Option<usize>,
    #[arg(long)]
    pub(crate) max_position: Option<u64>,
    #[arg(long)]
    pub(crate) interval_hours: Option<i64>,
    #[arg(long)]
    pub(crate) kelly_fraction: Option<f64>,
    #[arg(long)]
    pub(crate) max_position_pct: Option<f64>,
    #[arg(long)]
    pub(crate) take_profit: Option<f64>,
    #[arg(long)]
    pub(crate) stop_loss: Option<f64>,
    #[arg(long)]
    pub(crate) max_hold_hours: Option<i64>,
    #[arg(long)]
    pub(crate) data_source: Option<String>,
    #[arg(long)]
    pub(crate) attach: bool,
}

#[derive(Debug, Args)]
pub(crate) struct SessionsCommand {
    #[command(subcommand)]
    pub(crate) command: SessionsSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum SessionsSubcommand {
    List {
        #[arg(long, default_value_t = 25)]
        limit: u32,
    },
    Show {
        id: Option<String>,
    },
    Create(SessionCreateArgs),
    Stop,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum SessionModeArg {
    Paper,
    Backtest,
    Live,
}

impl SessionModeArg {
    pub(crate) fn as_daemon_mode(self) -> &'static str {
        match self {
            Self::Paper => "paper",
            Self::Backtest => "backtest",
            Self::Live => "live",
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct SessionCreateArgs {
    #[arg(long, value_enum, default_value_t = SessionModeArg::Paper)]
    pub(crate) mode: SessionModeArg,

    #[arg(long, default_value_t = 10_000.0)]
    pub(crate) initial_capital: f64,

    #[arg(long, default_value_t = 100)]
    pub(crate) max_positions: usize,

    #[arg(long, default_value_t = 0.25)]
    pub(crate) kelly_fraction: f64,

    #[arg(long, default_value_t = 0.10)]
    pub(crate) max_position_pct: f64,

    #[arg(long, default_value_t = 0.50)]
    pub(crate) take_profit_pct: f64,

    #[arg(long, default_value_t = 0.99)]
    pub(crate) stop_loss_pct: f64,

    #[arg(long, default_value_t = 48)]
    pub(crate) max_hold_hours: i64,

    #[arg(long, default_value_t = 2)]
    pub(crate) min_time_to_close_hours: i64,

    #[arg(long, default_value_t = 504)]
    pub(crate) max_time_to_close_hours: i64,

    #[arg(long, default_value_t = 0.20)]
    pub(crate) cash_reserve_pct: f64,

    #[arg(long, default_value_t = 5)]
    pub(crate) max_entries_per_tick: usize,

    #[arg(long, default_value_t = 0.07)]
    pub(crate) taker_rate: f64,

    #[arg(long, default_value_t = 0.0175)]
    pub(crate) maker_rate: f64,

    #[arg(long, default_value_t = 0.02)]
    pub(crate) max_fee_per_contract: f64,

    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub(crate) assume_taker: bool,

    #[arg(long, default_value_t = 0.02)]
    pub(crate) min_edge_after_fees: f64,

    #[arg(long, alias = "start")]
    pub(crate) backtest_start: Option<String>,

    #[arg(long, alias = "end")]
    pub(crate) backtest_end: Option<String>,

    #[arg(long, default_value_t = 1)]
    pub(crate) backtest_interval_hours: i64,
}

#[derive(Debug, Args)]
pub(crate) struct AuditCommand {
    #[command(subcommand)]
    pub(crate) command: AuditSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AuditSubcommand {
    List {
        #[arg(long, default_value_t = 100)]
        limit: u32,
    },
}

#[derive(Debug, Args)]
pub(crate) struct ConfigCommand {
    #[command(subcommand)]
    pub(crate) command: ConfigSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigSubcommand {
    Path,
    Show,
    Doctor,
}

#[derive(Debug, Args)]
pub(crate) struct ProfilesCommand {
    #[command(subcommand)]
    pub(crate) command: ProfilesSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ProfilesSubcommand {
    List,
    Show { name: Option<String> },
    Create(ProfileCreateArgs),
    SetDefault { name: String },
    Policy(ProfilePolicyArgs),
}

#[derive(Debug, Args)]
pub(crate) struct ProfileCreateArgs {
    pub(crate) name: String,

    #[arg(long)]
    pub(crate) daemon_url: Option<String>,

    #[arg(long)]
    pub(crate) kalshi_config: Option<PathBuf>,

    #[arg(long)]
    pub(crate) dry_run_default: bool,

    #[arg(long)]
    pub(crate) allow_live: bool,
}

#[derive(Debug, Args)]
pub(crate) struct ProfilePolicyArgs {
    pub(crate) name: Option<String>,

    #[arg(long)]
    pub(crate) allow_live: bool,

    #[arg(long)]
    pub(crate) deny_live: bool,

    #[arg(long)]
    pub(crate) dry_run_default: bool,

    #[arg(long)]
    pub(crate) no_dry_run_default: bool,

    #[arg(long)]
    pub(crate) max_position_usd: Option<f64>,

    #[arg(long)]
    pub(crate) max_bankroll_usd: Option<f64>,

    #[arg(long)]
    pub(crate) clear_max_position_usd: bool,

    #[arg(long)]
    pub(crate) clear_max_bankroll_usd: bool,

    #[arg(long)]
    pub(crate) require_yes: bool,

    #[arg(long)]
    pub(crate) no_require_yes: bool,
}
