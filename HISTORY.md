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

## 7 Mar 2025 Updates

### Stress Testing Implementation

- Added a new `StressTest` subcommand with configurable parameters for number of users and concurrency level
- Implemented batched processing to efficiently manage database connections
- Created a creative naming system combining medieval first names with Shakespearean last names
- Generated unique email addresses using UUID-based identifiers to prevent conflicts
- Added detailed performance metrics (total time, success/failure counts, insert rate)

### Timestamp Handling Improvement

- Migrated from `TIMESTAMP` to `TIMESTAMPTZ` for proper timezone support
- Updated schema to use `TIMESTAMPTZ` with `DEFAULT CURRENT_TIMESTAMP`
- Modified Rust code to use `chrono::DateTime<chrono::Utc>` instead of `NaiveDateTime`
- Ensured all timestamps are properly stored with UTC timezone information

### User Statistics Functionality

- Implemented a comprehensive `UserStats` command to analyze database content
- Added various statistical metrics:
  - Total user count and distribution by role
  - Newest and oldest user details
  - Creation time distribution by hour
  - Name length statistics (longest, shortest, average)
  - Popular first names and common email domains
  - User creation trends by date

### Performance Testing Results

- Conducted multiple stress tests with varying user counts and concurrency levels
- Achieved consistent throughput of ~18 users/second regardless of concurrency
- Demonstrated Aurora DSQL's ability to handle high concurrency (10-50 threads)
- Maintained 100% insert success rate across all tests
- Verified data integrity through statistical analysis

## Final Implementation

The final implementation successfully:
- Connects to Aurora DSQL using individual parameters from `.env` file
- Handles complex password characters with URL encoding
- Uses UUID for primary keys
- Implements a robust retry mechanism for handling transient database errors
- Includes proper error handling and logging
- Demonstrates complete database operations (create table, insert, query)
- Provides comprehensive stress testing capabilities
- Offers detailed statistical analysis of database content

This project serves as a good reference for connecting Rust applications to PostgreSQL-compatible cloud databases, especially those with unique compatibility requirements like Aurora DSQL. 