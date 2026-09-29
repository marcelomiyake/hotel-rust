use std::{env, net::SocketAddr};

use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "reservation_service=info".into()),
        )
        .init();
    let database_url = env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(16)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    let rate_url = env::var("RATE_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8082".into());
    let payment_url =
        env::var("PAYMENT_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8084".into());
    let token = env::var("STAFF_TOKEN").unwrap_or_else(|_| "local-staff-token-change-me".into());
    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse::<u16>()?;
    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(%address, "reservation service listening");
    axum::serve(
        listener,
        reservation_service::app(pool, rate_url, payment_url, token),
    )
    .await?;
    Ok(())
}
