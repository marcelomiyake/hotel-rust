use std::{env, net::SocketAddr};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "management_service=info".into()),
        )
        .init();
    let hotels_url =
        env::var("HOTEL_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8081".into());
    let rates_url = env::var("RATE_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8082".into());
    let reservations_url =
        env::var("RESERVATION_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8083".into());
    let payments_url =
        env::var("PAYMENT_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8084".into());
    let token = env::var("STAFF_TOKEN").unwrap_or_else(|_| "local-staff-token-change-me".into());
    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse::<u16>()?;
    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(%address, "management service listening");
    axum::serve(
        listener,
        management_service::app(hotels_url, rates_url, reservations_url, payments_url, token),
    )
    .await?;
    Ok(())
}
