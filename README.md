# Rust Aurora DSQL Demo with Authentication Tokens

A Rust application that demonstrates how to connect to an Amazon Aurora DSQL database using SQLx, including IAM authentication token generation.

## Prerequisites

- Rust and Cargo (https://www.rust-lang.org/tools/install)
- AWS credentials configured in your environment (~/.aws/credentials or environment variables)
- PostgreSQL client for local testing (psql)
- `libpq` development files (required by sqlx-postgres)
  - For Ubuntu/Debian: `sudo apt install libpq-dev`
  - For macOS: `brew install postgresql`

## Setup

1. Clone this repository
2. Update the `.env` file with your PostgreSQL credentials:

```
DB_HOST=your_cluster_endpoint.dsql.region.on.aws
DB_PORT=5432
DB_USER=admin
DB_NAME=postgres
```

Note: You no longer need to set DB_PASSWORD in the .env file, as this application now generates authentication tokens for connecting to Aurora DSQL.

## Running the Application

```bash
# List available commands
cargo run -- help

# Generate an authentication token for Aurora DSQL
cargo run -- generate-token

# Generate a token with custom parameters
cargo run -- generate-token --region us-east-1 --endpoint your-cluster.dsql.us-east-1.on.aws

# Generate a token for a non-admin user
cargo run -- generate-token --admin false

# Just output the token without extra information
cargo run -- generate-token --token-only
```

## Database Operations

```bash
# List users in the database
cargo run -- list-users

# Add a new user interactively
cargo run -- add-user

# Repopulate the database (WARNING: drops existing users table)
cargo run -- repopulate
```

## Features

- Connects to Aurora DSQL using SQLx
- Generates IAM authentication tokens for secure connections
- Supports both admin and regular user authentication
- Loads database credentials from a `.env` file
- Performs basic database operations (create tables, insert, query)

## Dependencies

- `sqlx` - Async SQL library with compile-time checked queries
- `aws-config` and `aws-sdk-dsql` - AWS SDK for Rust with Aurora DSQL support
- `dotenv` - For loading environment variables from a `.env` file
- `tokio` - Async runtime for Rust
- `clap` - Command line argument parsing
- `percent-encoding` - URL encoding of authentication tokens
- `dialoguer` - Interactive CLI utilities 