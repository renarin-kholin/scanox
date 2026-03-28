mod error;
mod orders;
mod razorpay;
mod types;
mod whatsapp;

use crate::config::Config;
use aes_gcm::aead::consts::U12;
use aes_gcm::aes::Aes256;
use aes_gcm::{Aes256Gcm, AesGcm, Key, KeyInit};
use anyhow::Context;
use axum::{Extension, Router};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
pub use error::{Error, ResultExt};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use types::ItemType;

pub type Result<T, E = Error> = std::result::Result<T, E>;
pub type UserNumer = String;

#[derive(Clone)]
struct ApiContext {
    config: Arc<Config>,
    db: PgPool,
    razorpay_items: HashMap<ItemType, RazorpayItem>,
    cipher: AesGcm<Aes256, U12>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct RazorpayItem {
    name: String,
    price: i32,
}
fn get_razorpay_items(config: &Config) -> HashMap<ItemType, RazorpayItem> {
    let mut items = HashMap::new();
    items.insert(
        ItemType::BW,
        RazorpayItem {
            name: "Black and White".to_string(),
            price: 100,
        },
    );
    items.insert(
        ItemType::BWT,
        RazorpayItem {
            name: "Black and White Two Sided".to_string(),
            price: 200,
        },
    );
    items.insert(
        ItemType::C,
        RazorpayItem {
            name: "Color".to_string(),
            price: 500,
        },
    );
    items.insert(
        ItemType::CT,
        RazorpayItem {
            name: "Color Two Sided".to_string(),
            price: 1000,
        },
    );
    items
}
fn load_aes_key(config: &Config) -> Key<Aes256Gcm> {
    let aes_key_encoded = &config.aes_key;
    let aes_key_decoded = BASE64_STANDARD
        .decode(aes_key_encoded)
        .expect("Could not decode the AES-GCM key.");
    *Key::<Aes256Gcm>::from_slice(&aes_key_decoded)
}
pub async fn serve(config: Config, db: PgPool) -> anyhow::Result<()> {
    let items = get_razorpay_items(&config);
    let aes_key = load_aes_key(&config);
    let cipher = Aes256Gcm::new(&aes_key);
    let port = config.port.clone();
    let app = api_router().layer(
        ServiceBuilder::new()
            .layer(Extension(ApiContext {
                config: Arc::new(config),
                razorpay_items: items,
                cipher,
                db,
            }))
            .layer(TraceLayer::new_for_http()),
    );
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    axum::serve(listener, app.into_make_service())
        .await
        .context("Error running http server")
}
fn api_router() -> Router {
    whatsapp::router()
        .merge(razorpay::router())
        .merge(orders::router())
}
