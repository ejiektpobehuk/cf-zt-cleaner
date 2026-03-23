mod cloudflare;
mod config;
mod error;
mod user;

use clap::{Parser, Subcommand};
use config::Config;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};
use tracing_subscriber::filter::LevelFilter;
use user::User;

/// Config template embedded at compile time
const CONFIG_TEMPLATE: &str = include_str!("../config.example.toml");

#[derive(Parser)]
#[command(name = "cf-zt-cleaner")]
#[command(about = "Reset CloudFlare Zero Trust users to a given permanent users list")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to configuration file
    #[arg(short, long, default_value = "config.toml", global = true)]
    config: PathBuf,

    /// Increase verbosity (can be repeated: -v for debug, -vv for trace)
    #[arg(short, long, action = clap::ArgAction::Count, conflicts_with = "quiet", global = true)]
    verbose: u8,

    /// Decrease verbosity (can be repeated: -q for warn, -qq for error)
    #[arg(short, long, action = clap::ArgAction::Count, conflicts_with = "verbose", global = true)]
    quiet: u8,

    /// Only target users whose last login is older than this duration (e.g. 2d, 48h).
    /// Users with no login record are always included.
    #[arg(long, value_name = "DURATION", global = true)]
    older_than: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Revoke Zero Trust seats for users not in the permanent list
    Clean {
        /// Auto-confirm seat revocation without prompting (for CI/CD)
        #[arg(long, conflicts_with = "interactive")]
        auto_confirm: bool,

        /// Interactively choose which users to revoke seats for one by one
        #[arg(short, long)]
        interactive: bool,
    },
    /// Preview what would be revoked without making any changes
    Preview,
    /// Initialize a new config.toml file with example configuration
    InitConfig {
        /// Output path for the config file (defaults to --config value)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Overwrite existing file if present
        #[arg(short, long)]
        force: bool,
    },
}

fn parse_duration(s: &str) -> anyhow::Result<chrono::Duration> {
    if let Some(days) = s.strip_suffix('d') {
        let n: i64 = days
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid number in duration '{}'", s))?;
        Ok(chrono::Duration::days(n))
    } else if let Some(hours) = s.strip_suffix('h') {
        let n: i64 = hours
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid number in duration '{}'", s))?;
        Ok(chrono::Duration::hours(n))
    } else {
        anyhow::bail!(
            "Invalid duration '{}'. Use Nd (days) or Nh (hours), e.g. 2d or 48h",
            s
        )
    }
}

fn confirm_seat_revocation(count: usize) -> anyhow::Result<bool> {
    print!(
        "Are you sure you want to revoke Zero Trust seats for {} user{}? [y/N]: ",
        count,
        if count == 1 { "" } else { "s" }
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let response = input.trim().to_lowercase();
    Ok(response == "y" || response == "yes")
}

/// Interactive mode - prompt for each user and revoke their seat immediately on approval
fn interactive_revoke_seats(
    users: &[User],
    client: &cloudflare::CloudFlareClient,
) -> anyhow::Result<()> {
    println!("\nInteractive mode: Review each user for seat revocation");
    println!("  [y]es    - revoke this user's seat");
    println!("  [n]o     - keep this user (default)");
    println!("  [a]ll    - revoke this and all remaining users' seats");
    println!("  [q]uit   - stop immediately\n");

    let mut revoked_count = 0;
    let mut skipped_count = 0;
    let mut error_count = 0;

    for (i, user) in users.iter().enumerate() {
        let last_login = user
            .last_successful_login
            .as_deref()
            .unwrap_or("never/unknown");
        print!(
            "[{}/{}] Revoke seat for {} ({}) last_successful_login={} ? [y/N/a/q]: ",
            i + 1,
            users.len(),
            user.email,
            user.id.as_deref().unwrap_or("no-id"),
            last_login
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let response = input.trim().to_lowercase();
        match response.as_str() {
            "y" | "yes" => {
                if revoke_seat_impl(user, client).is_ok() {
                    revoked_count += 1;
                } else {
                    error_count += 1;
                }
            }
            "a" | "all" => {
                // Revoke current and all remaining users' seats
                info!(
                    "  → Revoking seats for all remaining {} users...",
                    users.len() - i
                );
                for remaining_user in &users[i..] {
                    if revoke_seat_impl(remaining_user, client).is_ok() {
                        revoked_count += 1;
                    } else {
                        error_count += 1;
                    }
                }
                break;
            }
            "q" | "quit" => {
                info!("  → Quitting");
                break;
            }
            _ => {
                // Default is to skip (keep the user)
                info!("  → Keeping: {}", user.email);
                skipped_count += 1;
            }
        }
    }

    info!(
        "Interactive cleanup complete. Seats revoked: {}, Skipped: {}, Errors: {}",
        revoked_count, skipped_count, error_count
    );

    if error_count > 0 {
        anyhow::bail!(
            "Partial failure: {} user{} could not have seats revoked",
            error_count,
            if error_count == 1 { "" } else { "s" }
        );
    }

    Ok(())
}

fn init_config(output: &Path, force: bool) -> anyhow::Result<()> {
    if output.exists() && !force {
        anyhow::bail!(
            "Config file already exists at '{}'. Use --force to overwrite.",
            output.display()
        );
    }

    std::fs::write(output, CONFIG_TEMPLATE)?;
    println!("Created config file at '{}'", output.display());
    println!("Edit it with your CloudFlare credentials and permanent users list.");

    Ok(())
}

fn find_users_to_revoke(
    config_path: &Path,
) -> anyhow::Result<(Vec<User>, cloudflare::CloudFlareClient)> {
    info!("Loading configuration from: {}", config_path.display());
    let config = Config::load(config_path)?;

    let permanent_users: Vec<User> = config
        .users
        .permanent
        .into_iter()
        .map(User::from_email)
        .collect();

    info!(
        "Loaded {} permanent users from configuration",
        permanent_users.len()
    );
    if permanent_users.is_empty() {
        warn!(
            "Permanent users list is empty. All CloudFlare users with active seats are eligible for seat revocation."
        );
    }

    let client = cloudflare::CloudFlareClient::new(
        config.cloudflare.account_id,
        config.cloudflare.api_token,
    );

    info!("Fetching current users from CloudFlare...");
    let cloudflare_users = client.get_users()?;

    // Debug: show seat breakdown
    let total = cloudflare_users.len();
    let with_seat = cloudflare_users.iter().filter(|u| u.access_seat).count();
    debug!(
        "User breakdown: {} total, {} with active seats",
        total, with_seat
    );

    // Filter to only users with active Zero Trust seats
    let current_users: Vec<User> = cloudflare_users
        .into_iter()
        .filter(user::CloudFlareUser::has_active_seat)
        .filter_map(|cf_user| match User::try_from(cf_user) {
            Ok(user) => Some(user),
            Err(e) => {
                error!("Skipping CloudFlare user {}: missing email", e.user_id);
                None
            }
        })
        .collect();

    info!(
        "Found {} users with active seats in CloudFlare",
        current_users.len()
    );

    // Find users to revoke seats for (those not in permanent list)
    let mut users_to_revoke: Vec<User> = current_users
        .into_iter()
        .filter(|u| !u.is_in_permanent_list(&permanent_users))
        .collect();

    // Sort by "oldest last successful login" first so it's easy to spot who hasn't used the
    // service for the longest time. Treat missing login timestamps as oldest/unknown.
    users_to_revoke.sort_by(
        |a, b| match (&a.last_successful_login, &b.last_successful_login) {
            (None, None) => a.email.to_lowercase().cmp(&b.email.to_lowercase()),
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (Some(la), Some(lb)) => la
                .cmp(lb)
                .then_with(|| a.email.to_lowercase().cmp(&b.email.to_lowercase())),
        },
    );

    Ok((users_to_revoke, client))
}

fn print_users_to_revoke(users: &[User]) {
    info!("Found {} users to revoke seats for:", users.len());
    for user in users {
        let last_login = user
            .last_successful_login
            .as_deref()
            .unwrap_or("never/unknown");
        info!(
            "  - {} ({}) last_successful_login={}",
            user.email,
            user.id.as_deref().unwrap_or("no-id"),
            last_login
        );
    }
}

fn revoke_seat_impl(
    user: &User,
    client: &cloudflare::CloudFlareClient,
) -> std::result::Result<(), (String, String)> {
    user.seat_uid.as_ref().map_or_else(
        || {
            warn!(
                "Cannot revoke seat for user without seat_uid: {}",
                user.email
            );
            Err((user.email.clone(), "missing seat_uid".to_string()))
        },
        |seat_uid| match client.revoke_seat(seat_uid) {
            Ok(()) => {
                info!(
                    "Revoked seat for user: {} (id: {}, seat_uid: {})",
                    user.email,
                    user.id.as_deref().unwrap_or("none"),
                    seat_uid
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to revoke seat for user {} (seat_uid: {}): {}",
                    user.email, seat_uid, e
                );
                Err((user.email.clone(), e.to_string()))
            }
        },
    )
}

fn revoke_seats(users: &[User], client: &cloudflare::CloudFlareClient) -> anyhow::Result<()> {
    let mut revoked_count = 0;
    let mut error_count = 0;

    for user in users {
        if revoke_seat_impl(user, client).is_ok() {
            revoked_count += 1;
        } else {
            error_count += 1;
        }
    }

    info!(
        "Cleanup complete. Seats revoked: {}, Errors: {}",
        revoked_count, error_count
    );

    if error_count > 0 {
        anyhow::bail!(
            "Partial failure: {} user{} could not have seats revoked",
            error_count,
            if error_count == 1 { "" } else { "s" }
        );
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Handle init-config before setting up logging
    if let Commands::InitConfig { output, force } = cli.command {
        let output_path = output.unwrap_or(cli.config);
        return init_config(&output_path, force);
    }

    let level = match (cli.verbose, cli.quiet) {
        (2.., _) => LevelFilter::TRACE,
        (1, _) => LevelFilter::DEBUG,
        (0, 0) => LevelFilter::INFO,
        (_, 1) => LevelFilter::WARN,
        (_, 2..) => LevelFilter::ERROR,
    };

    tracing_subscriber::fmt().with_max_level(level).init();

    let (mut users_to_revoke, client) = find_users_to_revoke(&cli.config)?;

    if let Some(ref s) = cli.older_than {
        let threshold = parse_duration(s)?;
        let before = users_to_revoke.len();
        users_to_revoke.retain(|u| u.last_login_older_than(threshold));
        info!(
            "--older-than {}: {} of {} users match (last login older than {})",
            s,
            users_to_revoke.len(),
            before,
            s
        );
    }

    if users_to_revoke.is_empty() {
        info!("No seats to revoke. All current seat-holders are in the permanent list.");
        return Ok(());
    }

    print_users_to_revoke(&users_to_revoke);

    match cli.command {
        Commands::Preview => {
            warn!("Preview mode - no seats were revoked");
            Ok(())
        }
        Commands::Clean {
            auto_confirm,
            interactive,
        } => {
            if interactive {
                interactive_revoke_seats(&users_to_revoke, &client)
            } else {
                if !auto_confirm && !confirm_seat_revocation(users_to_revoke.len())? {
                    info!("Seat revocation cancelled by user");
                    return Ok(());
                }
                revoke_seats(&users_to_revoke, &client)
            }
        }
        Commands::InitConfig { .. } => unreachable!(),
    }
}
