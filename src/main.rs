use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use sqlx::types::{chrono, uuid::Uuid};
use std::env;
use std::error::Error;
use std::thread;
use std::time::Duration;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load environment variables from .env file
    dotenv().ok();
    
    // Get database connection details from environment variables
    let db_host = env::var("DB_HOST").expect("DB_HOST must be set in .env file");
    let db_port = env::var("DB_PORT").expect("DB_PORT must be set in .env file");
    let db_user = env::var("DB_USER").expect("DB_USER must be set in .env file");
    let db_password = env::var("DB_PASSWORD").expect("DB_PASSWORD must be set in .env file");
    let db_name = env::var("DB_NAME").expect("DB_NAME must be set in .env file");
    
    // URL encode the password to handle special characters
    let encoded_password = utf8_percent_encode(&db_password, NON_ALPHANUMERIC).to_string();
    
    // Construct the database URL with the encoded password
    let database_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        db_user, encoded_password, db_host, db_port, db_name
    );
    
    println!("Database URL constructed from parameters");
    
    // Create a connection pool
    println!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    
    println!("Connected successfully!");
    
    // Drop and recreate table with retry mechanism
    let max_retries = 3;
    let mut attempt = 0;
    
    loop {
        attempt += 1;
        println!("Attempt {}/{}: Dropping existing users table if it exists...", attempt, max_retries);
        
        let result = sqlx::query("DROP TABLE IF EXISTS users")
            .execute(&pool)
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
        .execute(&pool)
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
        ("David Miller", "david.miller@example.com", "Developer"),
        ("Emma Davis", "emma.davis@example.com", "User"),
        ("Frank Wilson", "frank.wilson@example.com", "Manager"),
        ("Grace Taylor", "grace.taylor@example.com", "Developer"),
        ("Henry Martin", "henry.martin@example.com", "User"),
        ("Ivy Chen", "ivy.chen@example.com", "Admin"),
        ("Jack Thompson", "jack.thompson@example.com", "User"),
        ("Kelly Anderson", "kelly.anderson@example.com", "Manager"),
        ("Leo Garcia", "leo.garcia@example.com", "Developer"),
        ("Maria Rodriguez", "maria.rodriguez@example.com", "User"),
    ];
    
    println!("Inserting sample users...");
    
    // Insert sample users with retry for each
    for (name, email, role) in sample_users {
        let user_id = Uuid::new_v4(); // Generate a new UUID for each user
        
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
            .execute(&pool)
            .await;
            
            match result {
                Ok(result) => {
                    if result.rows_affected() > 0 {
                        println!("User '{}' inserted with ID: {}", name, user_id);
                    } else {
                        println!("User '{}' already exists", email);
                    }
                    break;
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
    
    // Query all users with retry
    println!("Querying all users...");
    
    let mut query_attempt = 0;
    let max_query_retries = 3;
    let mut users = Vec::new();
    
    loop {
        query_attempt += 1;
        
        let result = sqlx::query(
            r#"
            SELECT id, name, email, role, created_at FROM users
            "#
        )
        .fetch_all(&pool)
        .await;
        
        match result {
            Ok(result) => {
                users = result;
                break;
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
    }
    
    println!("Found {} users in database", users.len());
    
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
    
    println!("Closing connection pool...");
    // Close the connection pool
    pool.close().await;
    println!("Connection closed");
    
    Ok(())
}
