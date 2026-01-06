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

