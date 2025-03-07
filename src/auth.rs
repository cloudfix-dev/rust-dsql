use aws_config::{BehaviorVersion, Region};
use aws_sdk_dsql::auth_token::{AuthTokenGenerator, Config};
use std::error::Error;

/// Generate an authentication token for Aurora DSQL
///
/// Args:
///   cluster_endpoint: The endpoint of the cluster (format: <cluster_id>.dsql.<region>.on.aws)
///   region: The AWS region (e.g. "us-east-1")
///   admin_user: Whether to generate a token for the admin user (true) or a regular user (false)
///
/// Returns:
///   A Result containing the authentication token as a String
pub async fn generate_auth_token(
    cluster_endpoint: &str,
    region: &str,
    admin_user: bool,
) -> Result<String, Box<dyn Error>> {
    // Load AWS configuration
    let sdk_config = aws_config::load_defaults(BehaviorVersion::latest()).await;

    // Create the AuthTokenGenerator with the cluster endpoint and region
    let signer = AuthTokenGenerator::new(
        Config::builder()
            .hostname(cluster_endpoint)
            .region(Region::new(region.to_string()))
            .build()
            .map_err(|e| e as Box<dyn Error>)?,
    );

    // Generate the appropriate token based on whether we're connecting as admin or not
    let token = if admin_user {
        signer.db_connect_admin_auth_token(&sdk_config).await
    } else {
        signer.db_connect_auth_token(&sdk_config).await
    };

    // Handle result and convert to string
    match token {
        Ok(token) => Ok(token.to_string()),
        Err(e) => Err(e as Box<dyn Error>),
    }
}

/// Generate a database connection string with authentication token
///
/// Args:
///   host: Database host (cluster endpoint)
///   port: Database port (usually 5432)
///   user: Database username
///   database: Database name
///   region: AWS region
///   admin_user: Whether to generate a token for the admin user
///
/// Returns:
///   A Result containing the connection string
#[allow(dead_code)]
pub async fn get_connection_string(
    host: &str,
    port: u16,
    user: &str,
    database: &str,
    region: &str,
    admin_user: bool,
) -> Result<String, Box<dyn Error>> {
    // Generate the auth token
    let token = generate_auth_token(host, region, admin_user).await?;

    // Create and return the connection string
    // Note: We use percent_encoding for the password to handle special characters
    let encoded_token =
        percent_encoding::utf8_percent_encode(&token, percent_encoding::NON_ALPHANUMERIC)
            .to_string();

    Ok(format!(
        "postgres://{}:{}@{}:{}/{}?sslmode=require",
        user, encoded_token, host, port, database
    ))
}
