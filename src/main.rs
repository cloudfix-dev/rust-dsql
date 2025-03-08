use clap::{Parser, Subcommand};
use dialoguer::{Confirm, Input};
use dotenv::dotenv;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::types::{chrono, uuid::Uuid};
use sqlx::Row;
use std::env;
use std::error::Error;
use std::thread;
use std::time::Duration;

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

    /// Stress test the database with parallel inserts
    StressTest {
        /// Number of users to insert (default: 100)
        #[arg(short, long, default_value_t = 100)]
        users: usize,

        /// Number of concurrent inserts (default: 10)
        #[arg(short, long, default_value_t = 10)]
        concurrency: usize,
    },

    /// Display statistics about users in the database
    UserStats,

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
async fn create_connection_pool() -> Result<PgPool, Box<dyn Error + Send + Sync>> {
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
async fn repopulate_database(pool: &PgPool) -> Result<(), Box<dyn Error + Send + Sync>> {
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
        println!(
            "Attempt {}/{}: Dropping existing users table if it exists...",
            attempt, max_retries
        );

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

        println!(
            "Attempt {}/{}: Creating users table with UUID primary key...",
            attempt, max_retries
        );

        let result = sqlx::query(
            r#"
            CREATE TABLE users (
                id UUID PRIMARY KEY,
                name VARCHAR(100) NOT NULL,
                email VARCHAR(100) UNIQUE NOT NULL,
                role VARCHAR(50) NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await;

        match result {
            Ok(_) => {
                println!("Table 'users' successfully created");
                break;
            }
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
    role: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
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
            }
            Err(err) => {
                println!(
                    "Error inserting user '{}' (attempt {}/{}): {}",
                    name, insert_attempt, max_insert_retries, err
                );

                if insert_attempt >= max_insert_retries {
                    return Err(err.into());
                }

                thread::sleep(Duration::from_millis(500));
            }
        }
    }
}

/// List all users in the database
async fn list_users(pool: &PgPool) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Querying all users...");

    let mut query_attempt = 0;
    let max_query_retries = 3;

    let users = loop {
        query_attempt += 1;

        let result = sqlx::query(
            r#"
            SELECT id, name, email, role, created_at FROM users
            "#,
        )
        .fetch_all(pool)
        .await;

        match result {
            Ok(result) => {
                break result;
            }
            Err(err) => {
                println!(
                    "Error querying users (attempt {}/{}): {}",
                    query_attempt, max_query_retries, err
                );

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
        // Use DateTime<Utc> instead of NaiveDateTime to match the TIMESTAMPTZ type
        println!(
            "ID: {}, Name: {}, Email: {}, Role: {}, Created at: {}",
            user.get::<Uuid, _>("id"),
            user.get::<String, _>("name"),
            user.get::<String, _>("email"),
            user.get::<String, _>("role"),
            user.get::<chrono::DateTime<chrono::Utc>, _>("created_at")
        );
    }

    Ok(())
}

/// Add a new user interactively
async fn add_user_interactive(pool: &PgPool) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Adding a new user. Please provide the following information:");

    let name: String = Input::new().with_prompt("Name").interact_text()?;

    let email: String = Input::new().with_prompt("Email").interact_text()?;

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
        }
        Err(e) => {
            println!("Failed to add user: {}", e);
            Err(e)
        }
    }
}

/// Stress test the database with parallel user inserts
async fn stress_test_database(pool: &PgPool, total_users: usize, concurrency: usize) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Starting stress test with {} users at concurrency level {}", total_users, concurrency);
    
    // Ensure the users table exists - fixed query to properly check table existence
    let table_exists = sqlx::query("SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_schema = 'public' AND table_name = 'users')")
        .fetch_one(pool)
        .await?
        .get::<bool, _>(0);
    
    if !table_exists {
        println!("The users table doesn't exist. Creating it...");
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id UUID PRIMARY KEY,
                name VARCHAR(100) NOT NULL,
                email VARCHAR(100) UNIQUE NOT NULL,
                role VARCHAR(50) NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await?;
        println!("Table 'users' created");
    }
    
    // Track performance metrics
    let start_time = std::time::Instant::now();
    let mut successful_inserts = 0;
    let mut failed_inserts = 0;
    
    // Generate random roles for variety
    let roles = vec!["User", "Admin", "Manager", "Guest", "Developer"];
    
    // Medieval first names
    let medieval_first_names = vec![
        "Aelfric", "Aldwin", "Baldwin", "Cedric", "Edmund", "Godfrey", "Harold", "Leofric",
        "Oswald", "Wilfrid", "Adelina", "Beatrice", "Cecily", "Eleanor", "Guinevere", "Isolde",
        "Matilda", "Rohesia", "Sybil", "Yvonne", "William", "Richard", "Robert", "Hugh", "Roland",
        "Giles", "Walter", "Henry", "Thomas", "John", "Agnes", "Alice", "Elaine", "Emma", "Joan",
        "Margaret", "Marian", "Edith", "Godiva", "Maud"
    ];
    
    // Shakespearean last names
    let shakespearean_last_names = vec![
        "Montague", "Capulet", "Othello", "Hamlet", "Macbeth", "Lear", "Prospero", "Oberon",
        "Puck", "Lysander", "Demetrius", "Titania", "Portia", "Shylock", "Malvolio", "Orsino",
        "Orlando", "Rosalind", "Falstaff", "Petruchio", "Ariel", "Caliban", "Polonius", "Laertes",
        "Ophelia", "Macduff", "Banquo", "Desdemona", "Cordelia", "Goneril", "Regan", "Kent",
        "Gloucester", "Albany", "Cornwall", "Feste", "Viola", "Sebastian", "Antonio", "Benvolio",
        "Mercutio", "Tybalt", "Horatio", "Fortinbras", "Bottom"
    ];
    
    // Process in batches of concurrency size
    for batch_idx in 0..(total_users / concurrency + 1) {
        let start_idx = batch_idx * concurrency;
        let end_idx = std::cmp::min(start_idx + concurrency, total_users);
        
        if start_idx >= total_users {
            break;
        }
        
        println!("Processing batch {}: users {}-{}", batch_idx + 1, start_idx + 1, end_idx);
        
        // Create a vector to hold our join handles
        let mut handles = Vec::new();
        
        // Start concurrent tasks
        for i in start_idx..end_idx {
            // Create unique test user data
            let user_id = Uuid::new_v4();
            
            // Generate random name from medieval first name and Shakespearean last name
            let first_name_idx = i % medieval_first_names.len();
            let last_name_idx = (i * 7) % shakespearean_last_names.len(); // Use a different multiplier to vary combinations
            let first_name = medieval_first_names[first_name_idx];
            let last_name = shakespearean_last_names[last_name_idx];
            let name = format!("{} {}", first_name, last_name);
            
            // Create email using the name with a unique identifier to avoid conflicts
            let email = format!("{}.{}.{}@kingdommail.com", 
                first_name.to_lowercase(), 
                last_name.to_lowercase(),
                user_id.simple());
            
            let role = roles[i % roles.len()];
            
            // Clone the pool for each task
            let pool = pool.clone();
            
            // Spawn a new task for this insert
            let handle = tokio::spawn(async move {
                let result = insert_user(&pool, user_id, &name, &email, role).await;
                (i, user_id, name, result)
            });
            
            handles.push(handle);
        }
        
        // Wait for all inserts in this batch to complete
        for handle in handles {
            match handle.await {
                Ok((i, user_id, name, result)) => {
                    match result {
                        Ok(_) => {
                            println!("Successfully inserted user #{} '{}' with ID: {}", i + 1, name, user_id);
                            successful_inserts += 1;
                        }
                        Err(e) => {
                            println!("Failed to insert user #{} '{}': {}", i + 1, name, e);
                            failed_inserts += 1;
                        }
                    }
                }
                Err(e) => {
                    println!("Task joining error: {}", e);
                    failed_inserts += 1;
                }
            }
        }
    }
    
    let elapsed = start_time.elapsed();
    let total_seconds = elapsed.as_secs_f64();
    let rate = successful_inserts as f64 / total_seconds;
    
    println!("\nStress Test Results:");
    println!("--------------------");
    println!("Total time: {:.2} seconds", total_seconds);
    println!("Successful inserts: {}", successful_inserts);
    println!("Failed inserts: {}", failed_inserts);
    println!("Insert rate: {:.2} users/second", rate);
    
    Ok(())
}

/// Get statistics about users in the database
async fn get_user_statistics(pool: &PgPool) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Gathering user statistics...");

    // Total user count
    let total_count = sqlx::query("SELECT COUNT(*) as count FROM users")
        .fetch_one(pool)
        .await?
        .get::<i64, _>("count");

    // Count by role
    let roles = sqlx::query(
        r#"
        SELECT role, COUNT(*) as count 
        FROM users 
        GROUP BY role 
        ORDER BY count DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    // Get newest and oldest user
    let newest_user = sqlx::query(
        r#"
        SELECT name, email, created_at 
        FROM users 
        ORDER BY created_at DESC 
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    let oldest_user = sqlx::query(
        r#"
        SELECT name, email, created_at 
        FROM users 
        ORDER BY created_at ASC 
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    // Get popular name prefixes
    let popular_names = sqlx::query(
        r#"
        SELECT LEFT(name, POSITION(' ' IN name)) as first_name, COUNT(*) as count
        FROM users
        WHERE POSITION(' ' IN name) > 0
        GROUP BY first_name
        ORDER BY count DESC
        LIMIT 5
        "#,
    )
    .fetch_all(pool)
    .await?;

    // Get user creation trends (users created by day)
    let creation_trends = sqlx::query(
        r#"
        SELECT 
            DATE(created_at) as date,
            COUNT(*) as count
        FROM users
        GROUP BY date
        ORDER BY date DESC
        LIMIT 7
        "#,
    )
    .fetch_all(pool)
    .await?;
    
    // Get creation time distribution (by hour of day)
    let hour_distribution = sqlx::query(
        r#"
        SELECT 
            EXTRACT(HOUR FROM created_at)::INT as hour,
            COUNT(*) as count
        FROM users
        GROUP BY hour
        ORDER BY hour
        "#,
    )
    .fetch_all(pool)
    .await?;
    
    // Get longest and shortest names
    let name_extremes = sqlx::query(
        r#"
        SELECT 
            (SELECT name FROM users ORDER BY LENGTH(name) DESC LIMIT 1) as longest_name,
            (SELECT name FROM users ORDER BY LENGTH(name) ASC LIMIT 1) as shortest_name,
            (SELECT AVG(LENGTH(name))::FLOAT8 FROM users) as avg_length
        "#,
    )
    .fetch_one(pool)
    .await?;

    // Email domain statistics
    let email_domains = sqlx::query(
        r#"
        SELECT 
            SUBSTRING(email FROM POSITION('@' IN email) + 1) as domain,
            COUNT(*) as count
        FROM users
        GROUP BY domain
        ORDER BY count DESC
        LIMIT 5
        "#,
    )
    .fetch_all(pool)
    .await?;

    // Print statistics
    println!("\n----- User Statistics -----");
    println!("Total users: {}", total_count);
    
    println!("\nDistribution by role:");
    for role in roles {
        println!(
            "- {}: {} users ({}%)",
            role.get::<String, _>("role"),
            role.get::<i64, _>("count"),
            (role.get::<i64, _>("count") as f64 / total_count as f64 * 100.0).round()
        );
    }

    if let Some(newest) = newest_user {
        println!(
            "\nNewest user: {} ({}) - Created: {}",
            newest.get::<String, _>("name"),
            newest.get::<String, _>("email"),
            newest.get::<chrono::DateTime<chrono::Utc>, _>("created_at")
        );
    }

    if let Some(oldest) = oldest_user {
        println!(
            "Oldest user: {} ({}) - Created: {}",
            oldest.get::<String, _>("name"),
            oldest.get::<String, _>("email"),
            oldest.get::<chrono::DateTime<chrono::Utc>, _>("created_at")
        );
    }

    // Print user creation trends
    println!("\nUser creation trends (last 7 days):");
    for trend in creation_trends {
        println!(
            "- {}: {} users",
            trend.get::<chrono::NaiveDate, _>("date"),
            trend.get::<i64, _>("count")
        );
    }
    
    // Print creation time distribution
    println!("\nCreation time distribution (by hour of day):");
    for hour in hour_distribution {
        let hour_val = hour.get::<i32, _>("hour");
        let count = hour.get::<i64, _>("count");
        let bar = "█".repeat((count as f64 / total_count as f64 * 50.0) as usize);
        println!(
            "- {:02}:00: {:4} users {}",
            hour_val,
            count,
            bar
        );
    }
    
    // Print name length information
    println!("\nName length statistics:");
    println!(
        "- Longest name: {} ({} chars)",
        name_extremes.get::<String, _>("longest_name"),
        name_extremes.get::<String, _>("longest_name").len()
    );
    println!(
        "- Shortest name: {} ({} chars)",
        name_extremes.get::<String, _>("shortest_name"),
        name_extremes.get::<String, _>("shortest_name").len()
    );
    println!(
        "- Average name length: {:.1} characters",
        name_extremes.get::<f64, _>("avg_length")
    );

    println!("\nMost common first names:");
    for name in popular_names {
        println!(
            "- {}: {} users",
            name.get::<String, _>("first_name"),
            name.get::<i64, _>("count")
        );
    }

    println!("\nMost common email domains:");
    for domain in email_domains {
        println!(
            "- {}: {} users ({}%)",
            domain.get::<String, _>("domain"),
            domain.get::<i64, _>("count"),
            (domain.get::<i64, _>("count") as f64 / total_count as f64 * 100.0).round()
        );
    }
    
    println!("\n---------------------------");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
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
        }
        Commands::ListUsers => {
            // Create the database connection pool
            let pool = create_connection_pool().await?;
            list_users(&pool).await?;
            // Close the connection pool
            println!("Closing connection pool...");
            pool.close().await;
            println!("Connection closed");
        }
        Commands::AddUser => {
            // Create the database connection pool
            let pool = create_connection_pool().await?;
            add_user_interactive(&pool).await?;
            // Close the connection pool
            println!("Closing connection pool...");
            pool.close().await;
            println!("Connection closed");
        }
        Commands::StressTest { users, concurrency } => {
            // Create the database connection pool
            let pool = create_connection_pool().await?;
            stress_test_database(&pool, users, concurrency).await?;
            // Close the connection pool
            println!("Closing connection pool...");
            pool.close().await;
            println!("Connection closed");
        }
        Commands::UserStats => {
            let pool = create_connection_pool().await?;
            get_user_statistics(&pool).await?;
            pool.close().await;
        }
        Commands::GenerateToken {
            region,
            endpoint,
            admin,
            token_only,
        } => {
            // Load environment variables
            dotenv().ok();

            // Use provided values or fall back to environment variables
            let region = region.unwrap_or_else(|| {
                let host = env::var("DB_HOST").expect("DB_HOST must be set in .env file");
                // Extract region from host - assuming format "<cluster_id>.dsql.<region>.on.aws"
                host.split('.').nth(2).unwrap_or("us-east-1").to_string()
            });

            let endpoint = endpoint
                .unwrap_or_else(|| env::var("DB_HOST").expect("DB_HOST must be set in .env file"));

            // Generate the token
            let token = auth::generate_auth_token(&endpoint, &region, admin).await?;

            if token_only {
                // Just print the token
                println!("{}", token);
            } else {
                // Get user and database name from env or use defaults
                let user = env::var("DB_USER").unwrap_or_else(|_| {
                    if admin {
                        "admin".to_string()
                    } else {
                        "postgres".to_string()
                    }
                });
                let database = env::var("DB_NAME").unwrap_or_else(|_| "postgres".to_string());
                let port = env::var("DB_PORT")
                    .unwrap_or_else(|_| "5432".to_string())
                    .parse::<u16>()
                    .unwrap_or(5432);

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
                println!(
                    "PGSSLMODE=require psql \"postgresql://{}@{}:{}/{}\" -W",
                    user, endpoint, port, database
                );
                println!("When prompted for password, use the token shown above.");
            }
        }
    }

    Ok(())
}
