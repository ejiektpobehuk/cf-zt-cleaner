# CloudFlare Zero Trust Cleaner

A Rust CLI tool to reset CloudFlare Zero Trust users to a given permanent list, helping to keep the number of users under a limit.

## How it works

1. Fetches current users from CloudFlare Access API
2. Compares against a local permanent users list defined in `config.toml`
3. Deletes users not in the permanent list

## Usage

```bash
# Initialize a config file from template
cf-zt-cleaner init-config

# Edit the configuration
$EDITOR config.toml

# Preview what would be deleted
cf-zt-cleaner preview

# Run the cleanup
cf-zt-cleaner clean

# Run without confirmation prompt (for CI/CD)
cf-zt-cleaner clean --auto-confirm
```

### Commands

```
Commands:
  clean        Clean up users not in the permanent list
  preview      Preview what would be deleted without actually deleting
  init-config  Initialize a new config.toml file with example configuration
  help         Print this message or the help of the given subcommand(s)
```

### Global Options

```
Options:
  -c, --config <CONFIG>  Path to configuration file [default: config.toml]
  -v, --verbose          Increase verbosity (-v for debug, -vv for trace)
  -q, --quiet            Decrease verbosity (-q for warn, -qq for error)
  -h, --help             Print help
```

## Configuration

Create a `config.toml` file based on [`config.example.toml`](config.example.toml).
You can either copy it manually from the repository or generate it locally with `cf-zt-cleaner init-config`.

### Environment Variables

CloudFlare credentials can also be provided via environment variables:

| Variable | Description |
|----------|-------------|
| `CF_ACCOUNT_ID` | CloudFlare account ID |
| `CF_API_TOKEN` | CloudFlare API token |

**Priority**: Environment variables take precedence over config file values. If both are set, a warning is displayed and the environment variable value is used.

```bash
# Example: using environment variables
export CF_ACCOUNT_ID="your-account-id"
export CF_API_TOKEN="your-api-token"
cf-zt-cleaner preview
```

### Getting CloudFlare credentials

1. **Account ID**: Found in your CloudFlare dashboard URL or in Account Settings
2. **API Token**: Create at https://dash.cloudflare.com/profile/api-tokens
   - Required permissions: `Access: Users` (Read, Edit)

## Building

```bash
cargo build --release
```

The binary will be at `target/release/cf-zt-cleaner`.

## Logging

Use `-v` for debug output or `-vv` for trace output:

```bash
cf-zt-cleaner -v preview
cf-zt-cleaner -vv clean
```

