# CloudFlare Zero Trust Cleaner

A Rust CLI tool to reset CloudFlare Zero Trust users to a given permanent list, helping to keep the number of users under a limit.

## How it works

1. Fetches current users from CloudFlare Access API
2. Compares against a local permanent users list defined in `config.toml`
3. Deletes users not in the permanent list

## Usage

```bash
# Copy and edit the configuration file
cp config.example.toml config.toml

# Run in dry-run mode to see what would be deleted
cf-zt-cleaner --dry-run

# Run for real
cf-zt-cleaner
```

### CLI Options

```
Options:
  -c, --config <CONFIG>  Path to configuration file [default: config.toml]
  -d, --dry-run          Dry run mode - show what would be deleted
  -h, --help             Print help
```

## Configuration

Create a `config.toml` file (see `config.example.toml` for reference):

```toml
[cloudflare]
account_id = "your-account-id-here"
api_token = "your-api-token-here"

[users]
permanent = [
    { email = "admin@example.com" },
    { email = "developer@example.com" },
]
```

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
cf-zt-cleaner --dry-run
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

Set the `RUST_LOG` environment variable for more detailed output:

```bash
RUST_LOG=debug cf-zt-cleaner --dry-run
```

