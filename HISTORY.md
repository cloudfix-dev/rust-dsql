# Development History

This document outlines the evolution of the Rust PostgreSQL application, highlighting key challenges faced and solutions implemented.

## Initial Setup

- Created a new Rust project with Cargo
- Added dependencies for SQLx, Tokio, and dotenv
- Implemented basic PostgreSQL connection with SQLx
- Used `.env` file to store database connection details as `DATABASE_URL`
- Created a simple users table with basic CRUD operations

## Connection String Evolution

- Modified the code to dynamically construct the connection string instead of using a static `DATABASE_URL`
- Extracted individual connection parameters (`DB_HOST`, `DB_PORT`, `DB_USER`, `DB_PASSWORD`, `DB_NAME`) from the `.env` file
- Constructed PostgreSQL connection string using the format: `postgres://username:password@host:port/database`

## Aurora DSQL Compatibility

- Encountered compatibility issues with Amazon Aurora DSQL
- Modified SQL schema to work with Aurora DSQL compatibility constraints:
  - Replaced `SERIAL` type (not supported) with explicit ID handling
  - Adjusted timestamp handling from `TIMESTAMPTZ` to `TIMESTAMP`
  - Updated SQL syntax to be compatible with Aurora limitations

## UUID Implementation

- Added UUID feature to SQLx in Cargo.toml
- Modified schema to use UUID as primary key instead of numeric IDs
- Implemented UUID v4 generation for new user records
- Added a `role` field to the users table for more comprehensive sample data

## Error Handling & Retries

- Encountered transient errors common in cloud databases (`40001` error code)
- First attempt: Implemented a generic retry function with closures (encountered lifetime issues)
- Final solution: Implemented direct retry loops for each database operation
- Added proper error logging and maximum retry limits
- Included delay between retry attempts to allow system recovery

## Data Type Compatibility

- Fixed timestamp handling by switching from `DateTime<Utc>` to `NaiveDateTime` to match Aurora's `TIMESTAMP` type
- Adjusted data retrieval to correctly handle UUID and timestamp formats
- Ensured compatibility between Rust types and database column types

## Sample Data Generation

- Added code to create multiple sample users with different roles
- Implemented conflict handling for duplicate entries
- Added comprehensive logging of database operations

## Final Implementation

The final implementation successfully:
- Connects to Aurora DSQL using individual parameters from `.env` file
- Handles complex password characters with URL encoding
- Uses UUID for primary keys
- Implements a robust retry mechanism for handling transient database errors
- Includes proper error handling and logging
- Demonstrates complete database operations (create table, insert, query)

This project serves as a good reference for connecting Rust applications to PostgreSQL-compatible cloud databases, especially those with unique compatibility requirements like Aurora DSQL. 