mod cloudflare;
mod config;
mod error;
mod user;

use clap::{Parser, Subcommand};
use config::Config;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};
use tracing_subscriber::filter::LevelFilter;
use user::User;

/// Config template embedded at compile time
const CONFIG_TEMPLATE: &str = include_str!("../config.example.toml");

#[derive(Parser)]
#[command(name = "cf-zt-cleaner")]
#[command(about = "Reset CloudFlare Zero Trust users to a given permanent users list")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to configuration file
    #[arg(short, long, default_value = "config.toml", global = true)]
    config: PathBuf,

    /// Dry run mode - show what would be deleted without actually deleting
    #[arg(short, long, global = true)]
    dry_run: bool,

    /// Auto-confirm deletion without prompting (for CI/CD)
    #[arg(long, global = true)]
    auto_confirm: bool,

    /// Increase verbosity (can be repeated: -v for debug, -vv for trace)
    #[arg(short, long, action = clap::ArgAction::Count, conflicts_with = "quiet", global = true)]
    verbose: u8,

    /// Decrease verbosity (can be repeated: -q for warn, -qq for error)
    #[arg(short, long, action = clap::ArgAction::Count, conflicts_with = "verbose", global = true)]
    quiet: u8,
}

#[derive(Subcommand)]
enum Commands {
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

fn confirm_deletion(count: usize) -> anyhow::Result<bool> {
    print!(
        "Are you sure you want to delete {} user{}? [y/N]: ",
        count,
        if count == 1 { "" } else { "s" }
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let response = input.trim().to_lowercase();
    Ok(response == "y" || response == "yes")
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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Handle init-config before setting up logging
    if let Some(Commands::InitConfig { output, force }) = cli.command {
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

    info!("Loading configuration from: {}", cli.config.display());
    let config = Config::load(&cli.config)?;

    let permanent_users: Vec<User> = config.users.permanent.into_iter().map(User::from).collect();

    info!(
        "Loaded {} permanent users from configuration",
        permanent_users.len()
    );

    let client = cloudflare::CloudFlareClient::new(
        config.cloudflare.account_id,
        config.cloudflare.api_token,
    );

    info!("Fetching current users from CloudFlare...");
    let cloudflare_users = client.get_users()?;

    let current_users: Vec<User> = cloudflare_users
        .into_iter()
        .filter_map(|cf_user| match User::try_from(cf_user) {
            Ok(user) => Some(user),
            Err(e) => {
                error!("Skipping CloudFlare user {}: missing email", e.user_id);
                None
            }
        })
        .collect();

    info!("Found {} users in CloudFlare", current_users.len());

    // Find users to delete (those not in permanent list)
    let users_to_delete: Vec<&User> = current_users
        .iter()
        .filter(|u| !u.is_in_permanent_list(&permanent_users))
        .collect();

    if users_to_delete.is_empty() {
        info!("No users to delete. All current users are in the permanent list.");
        return Ok(());
    }

    info!("Found {} users to delete:", users_to_delete.len());
    for user in &users_to_delete {
        info!(
            "  - {} ({})",
            user.email,
            user.id.as_deref().unwrap_or("no-id")
        );
    }

    if cli.dry_run {
        warn!("Dry run mode - no users were deleted");
        return Ok(());
    }

    // Prompt for confirmation unless --auto-confirm flag is provided
    if !cli.auto_confirm && !confirm_deletion(users_to_delete.len())? {
        info!("Deletion cancelled by user");
        return Ok(());
    }

    // Delete users not in permanent list
    let mut deleted_count = 0;
    let mut error_count = 0;

    for user in users_to_delete {
        if let Some(id) = &user.id {
            match client.delete_user(id) {
                Ok(()) => {
                    info!("Deleted user: {} ({})", user.email, id);
                    deleted_count += 1;
                }
                Err(e) => {
                    error!("Failed to delete user {} ({}): {}", user.email, id, e);
                    error_count += 1;
                }
            }
        } else {
            warn!("Cannot delete user without ID: {}", user.email);
        }
    }

    info!(
        "Cleanup complete. Deleted: {}, Errors: {}",
        deleted_count, error_count
    );

    if error_count > 0 {
        anyhow::bail!(
            "Partial failure: {} user{} could not be deleted",
            error_count,
            if error_count == 1 { "" } else { "s" }
        );
    }

    Ok(())
}
