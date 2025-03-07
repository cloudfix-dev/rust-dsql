# Rust PostgreSQL Demo with SQLx

A simple Rust application that demonstrates how to connect to a PostgreSQL database using SQLx and environment variables.

## Prerequisites

- Rust and Cargo (https://www.rust-lang.org/tools/install)
- PostgreSQL server running locally or remotely
- `libpq` development files (required by sqlx-postgres)
  - For Ubuntu/Debian: `sudo apt install libpq-dev`
  - For macOS: `brew install postgresql`

## Setup

1. Clone this repository
2. Update the `.env` file with your PostgreSQL credentials:

```
DB_HOST=your_host
DB_PORT=5432
DB_USER=your_username
DB_PASSWORD=your_password
DB_NAME=your_database
```

## Running the Application

```bash
cargo run
```

## Features

- Connects to PostgreSQL using SQLx
- Loads database credentials from individual parameters in a `.env` file
- Constructs the connection string dynamically
- Creates a `users` table if it doesn't exist
- Inserts a sample user
- Queries and displays all users

## Dependencies

- `sqlx` - Async SQL library with compile-time checked queries
- `dotenv` - For loading environment variables from a `.env` file
- `tokio` - Async runtime for Rust 