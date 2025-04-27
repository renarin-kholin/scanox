mod error;
mod orders;
mod whatsapp;
mod types;
mod razorpay;

use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Context;
use axum::{Extension, Router};
use sqlx::PgPool;
use tower::ServiceBuilder;
pub use error::{Error, ResultExt};
use crate::config::Config;
use tower_http::trace::TraceLayer;
use types::ItemType;

pub type Result<T, E=Error> = std::result::Result<T, E>;
pub type UserNumer = String;

#[derive(Clone)]
struct ApiContext {
    config: Arc<Config>,
    db: PgPool,
    razorpay_items: HashMap<ItemType, String>
}
fn get_razorpay_items(config: &Config) -> HashMap<ItemType, String> {
    let mut items = HashMap::new();
    items.insert(ItemType::BW, config.item_bw.clone());
    items.insert(ItemType::BWT, config.item_bw_t.clone());
    items.insert(ItemType::C, config.item_c.clone());
    items.insert(ItemType::CT, config.item_c_t.clone());
    items

}
pub async fn serve(config: Config, db: PgPool) -> anyhow::Result<()> {
    let items = get_razorpay_items(&config);
    let port = config.port.clone();
    let app = api_router().layer(
        ServiceBuilder::new()
            .layer(Extension(ApiContext {
                config: Arc::new(config),
                razorpay_items: items,
                
                db
            }))
            .layer(TraceLayer::new_for_http())
    );
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    axum::serve(listener, app.into_make_service()).await.context("Error running http server")
}
fn api_router() -> Router {
    whatsapp::router()
        .merge(razorpay::router())
        .merge(orders::router())
}