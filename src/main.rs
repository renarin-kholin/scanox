use anyhow::Context;
use clap::Parser;
use sqlx::postgres::PgPoolOptions;
use scanox_backend::config::Config;
use scanox_backend::http;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    env_logger::init();
    let config = Config::parse();
    let db = PgPoolOptions::new()
        .max_connections(50)
        .connect(&config.database_url)
        .await
        .context("Could not connect to database_url")?;
    sqlx::migrate!().run(&db).await?;
    http::serve(config, db).await?;
    Ok(())
}