use clap::{Parser, Subcommand};
use dialoguer::{Confirm, Input};
use dotenv::dotenv;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;
use sqlx::types::{chrono, uuid::Uuid};
use std::env;
use std::error::Error;
use std::thread;
use std::time::Duration;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

// Add the auth module
mod auth;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Repopulate the database (WARNING: drops existing users table)
    Repopulate,
    
    /// List all users in the database
    ListUsers,
    
    /// Add a new user interactively
    AddUser,

    /// Generate an authentication token for Aurora DSQL
    GenerateToken {
        /// The AWS region (e.g., "us-east-1")
        #[arg(short, long)]
        region: Option<String>,
        
        /// The cluster endpoint
        #[arg(short, long)]
        endpoint: Option<String>,
        
        /// Generate a token for the admin user (default: true)
        #[arg(short, long, default_value_t = true)]
        admin: bool,
        
        /// Just print the token (don't include connection details)
        #[arg(short, long, default_value_t = false)]
        token_only: bool,
    },
}

/// Create a database connection pool using parameters from .env file
async fn create_connection_pool() -> Result<PgPool, Box<dyn Error>> {
    // Load environment variables from .env file
    dotenv().ok();
    
    // Get database connection details from environment variables
    let db_host = env::var("DB_HOST").expect("DB_HOST must be set in .env file");
    let db_port = env::var("DB_PORT").expect("DB_PORT must be set in .env file");
    let db_user = env::var("DB_USER").expect("DB_USER must be set in .env file");
    let db_name = env::var("DB_NAME").expect("DB_NAME must be set in .env file");
    
    // Extract region from host
    let region = String::from("us-east-1");
    
    println!("Generating auth token for connection...");
    
    // Determine if we should use admin auth based on the username
    let admin_auth = db_user.to_lowercase() == "admin";
    
    // Generate the authentication token
    let auth_token = auth::generate_auth_token(&db_host, &region, admin_auth).await?;
    
    // URL encode the token to handle special characters
    let encoded_token = utf8_percent_encode(&auth_token, NON_ALPHANUMERIC).to_string();
    
    // Construct the database URL with the encoded token
    let database_url = format!(
        "postgres://{}:{}@{}:{}/{}?sslmode=require",
        db_user, encoded_token, db_host, db_port, db_name
    );
    
    println!("Database URL constructed from parameters");
    
    // Create a connection pool
    println!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    
    println!("Connected successfully!");
    
    Ok(pool)
}

/// Repopulate the database with sample data
async fn repopulate_database(pool: &PgPool) -> Result<(), Box<dyn Error>> {
    // Confirm with the user before proceeding
    let confirmed = Confirm::new()
        .with_prompt("WARNING: This will drop the existing users table and all its data. Continue?")
        .default(false)
        .interact()?;
    
    if !confirmed {
        println!("Operation cancelled");
        return Ok(());
    }
    
    // Drop and recreate table with retry mechanism
    let max_retries = 3;
    let mut attempt = 0;
    
    loop {
        attempt += 1;
        println!("Attempt {}/{}: Dropping existing users table if it exists...", attempt, max_retries);
        
        let result = sqlx::query("DROP TABLE IF EXISTS users")
            .execute(pool)
            .await;
            
        if let Err(err) = result {
            println!("Error dropping table: {}", err);
            if attempt >= max_retries {
                return Err(err.into());
            }
            thread::sleep(Duration::from_millis(500));
            continue;
        }
        
        println!("Attempt {}/{}: Creating users table with UUID primary key...", attempt, max_retries);
        
        let result = sqlx::query(
            r#"
            CREATE TABLE users (
                id UUID PRIMARY KEY,
                name VARCHAR(100) NOT NULL,
                email VARCHAR(100) UNIQUE NOT NULL,
                role VARCHAR(50) NOT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await;
        
        match result {
            Ok(_) => {
                println!("Table 'users' successfully created");
                break;
            },
            Err(err) => {
                println!("Error creating table: {}", err);
                if attempt >= max_retries {
                    return Err(err.into());
                }
                thread::sleep(Duration::from_millis(500));
            }
        }
    }
    
    // Sample data to insert
    let sample_users = vec![
        ("John Doe", "john.doe@example.com", "Admin"),
        ("Jane Smith", "jane.smith@example.com", "User"),
        ("Bob Johnson", "bob.johnson@example.com", "User"),
        ("Alice Williams", "alice.williams@example.com", "Manager"),
        ("Charlie Brown", "charlie.brown@example.com", "User"),
    ];
    
    println!("Inserting sample users...");
    
    // Insert sample users with retry for each
    for (name, email, role) in sample_users {
        let user_id = Uuid::new_v4(); // Generate a new UUID for each user
        
        match insert_user(pool, user_id, name, email, role).await {
            Ok(_) => println!("User '{}' inserted with ID: {}", name, user_id),
            Err(e) => println!("Failed to insert user '{}': {}", name, e),
        }
    }
    
    println!("Database has been repopulated successfully");
    
    Ok(())
}

/// Insert a new user into the database
async fn insert_user(
    pool: &PgPool, 
    user_id: Uuid,
    name: &str,
    email: &str,
    role: &str
) -> Result<(), Box<dyn Error>> {
    let mut insert_attempt = 0;
    let max_insert_retries = 3;
    
    loop {
        insert_attempt += 1;
        
        let result = sqlx::query(
            r#"
            INSERT INTO users (id, name, email, role) 
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (email) DO NOTHING
            "#,
        )
        .bind(user_id)
        .bind(name)
        .bind(email)
        .bind(role)
        .execute(pool)
        .await;
        
        match result {
            Ok(result) => {
                if result.rows_affected() > 0 {
                    return Ok(());
                } else {
                    return Err(format!("User with email '{}' already exists", email).into());
                }
            },
            Err(err) => {
                println!("Error inserting user '{}' (attempt {}/{}): {}", 
                        name, insert_attempt, max_insert_retries, err);
                
                if insert_attempt >= max_insert_retries {
                    return Err(err.into());
                }
                
                thread::sleep(Duration::from_millis(500));
            }
        }
    }
}

/// List all users in the database
async fn list_users(pool: &PgPool) -> Result<(), Box<dyn Error>> {
    println!("Querying all users...");
    
    let mut query_attempt = 0;
    let max_query_retries = 3;
    
    let users = loop {
        query_attempt += 1;
        
        let result = sqlx::query(
            r#"
            SELECT id, name, email, role, created_at FROM users
            "#
        )
        .fetch_all(pool)
        .await;
        
        match result {
            Ok(result) => {
                break result;
            },
            Err(err) => {
                println!("Error querying users (attempt {}/{}): {}", 
                         query_attempt, max_query_retries, err);
                
                if query_attempt >= max_query_retries {
                    return Err(err.into());
                }
                
                thread::sleep(Duration::from_millis(500));
            }
        }
    };
    
    println!("Found {} users in database", users.len());
    
    if users.is_empty() {
        println!("No users found in the database.");
        return Ok(());
    }
    
    println!("\nUsers in database:");
    for user in users {
        // Use NaiveDateTime instead of DateTime<Utc> to match the TIMESTAMP type
        println!(
            "ID: {}, Name: {}, Email: {}, Role: {}, Created at: {}", 
            user.get::<Uuid, _>("id"),
            user.get::<String, _>("name"), 
            user.get::<String, _>("email"),
            user.get::<String, _>("role"),
            user.get::<chrono::NaiveDateTime, _>("created_at")
        );
    }
    
    Ok(())
}

/// Add a new user interactively
async fn add_user_interactive(pool: &PgPool) -> Result<(), Box<dyn Error>> {
    println!("Adding a new user. Please provide the following information:");
    
    let name: String = Input::new()
        .with_prompt("Name")
        .interact_text()?;
    
    let email: String = Input::new()
        .with_prompt("Email")
        .interact_text()?;
    
    let role: String = Input::new()
        .with_prompt("Role (Admin/User/Manager)")
        .default("User".into())
        .interact_text()?;
    
    let user_id = Uuid::new_v4();
    
    match insert_user(pool, user_id, &name, &email, &role).await {
        Ok(_) => {
            println!("User added successfully!");
            println!("User ID: {}", user_id);
            println!("Name: {}", name);
            println!("Email: {}", email);
            println!("Role: {}", role);
            Ok(())
        },
        Err(e) => {
            println!("Failed to add user: {}", e);
            Err(e)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    
    // Execute the appropriate command
    match cli.command {
        Commands::Repopulate => {
            // Create the database connection pool
            let pool = create_connection_pool().await?;
            repopulate_database(&pool).await?;
            // Close the connection pool
            println!("Closing connection pool...");
            pool.close().await;
            println!("Connection closed");
        },
        Commands::ListUsers => {
            // Create the database connection pool
            let pool = create_connection_pool().await?;
            list_users(&pool).await?;
            // Close the connection pool
            println!("Closing connection pool...");
            pool.close().await;
            println!("Connection closed");
        },
        Commands::AddUser => {
            // Create the database connection pool
            let pool = create_connection_pool().await?;
            add_user_interactive(&pool).await?;
            // Close the connection pool
            println!("Closing connection pool...");
            pool.close().await;
            println!("Connection closed");
        },
        Commands::GenerateToken { region, endpoint, admin, token_only } => {
            // Load environment variables
            dotenv().ok();
            
            // Use provided values or fall back to environment variables
            let region = region.unwrap_or_else(|| {
                let host = env::var("DB_HOST").expect("DB_HOST must be set in .env file");
                // Extract region from host - assuming format "<cluster_id>.dsql.<region>.on.aws"
                host.split('.')
                    .nth(2)
                    .unwrap_or("us-east-1")
                    .to_string()
            });
            
            let endpoint = endpoint.unwrap_or_else(|| {
                env::var("DB_HOST").expect("DB_HOST must be set in .env file")
            });
            
            // Generate the token
            let token = auth::generate_auth_token(&endpoint, &region, admin).await?;
            
            if token_only {
                // Just print the token
                println!("{}", token);
            } else {
                // Get user and database name from env or use defaults
                let user = env::var("DB_USER").unwrap_or_else(|_| {
                    if admin { "admin".to_string() } else { "postgres".to_string() }
                });
                let database = env::var("DB_NAME").unwrap_or_else(|_| "postgres".to_string());
                let port = env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string())
                    .parse::<u16>().unwrap_or(5432);
                
                // Print connection details
                println!("Authentication token generated successfully!");
                println!("Host:     {}", endpoint);
                println!("Port:     {}", port);
                println!("User:     {}", user);
                println!("Database: {}", database);
                println!("Region:   {}", region);
                println!("Admin:    {}", if admin { "Yes" } else { "No" });
                println!("\nToken: {}", token);
                
                // Print a sample connection command
                println!("\nSample connection command:");
                println!("PGSSLMODE=require psql \"postgresql://{}@{}:{}/{}\" -W", 
                    user, endpoint, port, database);
                println!("When prompted for password, use the token shown above.");
            }
        }
    }
    
    Ok(())
}
