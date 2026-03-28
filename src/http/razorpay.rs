use crate::http::whatsapp::{SendMessageType, send_message};
use crate::http::{ApiContext, Error, Result};
use aes_gcm::aead::consts::U12;
use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::aes::Aes256;
use aes_gcm::{AeadCore, Aes256Gcm, AesGcm, Key, Nonce};
use anyhow::anyhow;
use axum::routing::post;
use axum::{Extension, Router};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use qrcode_generator::QrCodeEcc;
use std::str::from_utf8;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::multipart;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::Instant;

pub fn router() -> Router {
    Router::new().route("/webhook/razorpay", post(post_razorpay))
}
#[derive(Deserialize)]
struct RazorPayWebhookEventPayloadInvoiceEntity {
    order_id: String,
    // status: String,
}
#[derive(Deserialize)]
struct RazorPayWebhookEventPayloadInvoice {
    entity: RazorPayWebhookEventPayloadInvoiceEntity,
}
#[derive(Deserialize)]
struct RazorPayWebhookEventPayload {
    invoice: RazorPayWebhookEventPayloadInvoice,
}
#[derive(Deserialize)]
struct RazorPayWebhookEvent {
    // event: String,
    payload: RazorPayWebhookEventPayload,
}
pub async fn generate_qrcode(hash: String) -> Result<Vec<u8>> {
    qrcode_generator::to_png_to_vec(hash, QrCodeEcc::Low, 1024).map_err(|_| Error::NotFound)
}
#[derive(Serialize, Deserialize)]
pub struct QRPayloadData {
    pub created_at: SystemTime,
    pub order_id: String,
}
#[derive(Serialize, Deserialize)]
pub struct QRPayload {
    pub hash: String,
    pub nonce: String,
}
pub async fn encrypt_order(order_id: String, cipher: &AesGcm<Aes256, U12>) -> Result<String> {
    // Ok( json!({
    //     "order_id": order_id,
    // }).to_string())
    let current_time = SystemTime::now();
    let payload_data = QRPayloadData {
        created_at: current_time,
        order_id,
    };
    let payload_data_str = serde_json::to_string(&payload_data)
        .map_err(|_| anyhow!("Could not serialize payload to string"))?;

    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let nonce_encoded = BASE64_STANDARD.encode(nonce);

    let ciphertext = cipher
        .encrypt(&nonce, payload_data_str.as_bytes())
        .map_err(|_| anyhow!("Could not encrypt QR payload"))?;
    let ciphertext_encoded = BASE64_STANDARD.encode(ciphertext);
    let qr_payload = QRPayload {
        hash: ciphertext_encoded,
        nonce: nonce_encoded,
    };
    let qr_payload_str = serde_json::to_string(&qr_payload)
        .map_err(|_| anyhow!("Could not convert payload to string"))?;
    Ok(qr_payload_str)
}
pub async fn decrypt_order(
    cipher: &AesGcm<Aes256, U12>,
    encrypted_message: String,
) -> Result<QRPayloadData> {
    let qr_payload: QRPayload = serde_json::from_str(&encrypted_message)
        .map_err(|_| anyhow!("Encrypted message is not in valid format."))?;
    let nonce = BASE64_STANDARD
        .decode(qr_payload.nonce)
        .map_err(|_| anyhow!("Could not decode nonce"))?;
    let hash = BASE64_STANDARD
        .decode(qr_payload.hash)
        .map_err(|_| anyhow!("could not decode hash"))?;

    let nonce = Nonce::from_slice(&nonce);
    let decrypted = cipher
        .decrypt(&nonce, hash.as_ref())
        .map_err(|_| anyhow!("Could not decrypt hash"))?;
    let payload_data_str =
        from_utf8(&decrypted).map_err(|_| anyhow!("Could not decode utf8 string"))?;
    let payload_data: QRPayloadData = serde_json::from_str(&payload_data_str)
        .map_err(|_| anyhow!("Could not parse JSON for QR payload"))?;
    Ok(payload_data)
}
#[derive(Deserialize)]
struct MediaUploadResponse {
    id: String,
}
pub(crate) async fn upload_image(
    image: Vec<u8>,
    razorpay_order_id: String,
    token: &str,
) -> Result<String> {
    let client = reqwest::Client::new();
    let part = multipart::Part::bytes(image)
        .file_name(format!("{}.png", razorpay_order_id))
        .mime_str("image/png")
        .map_err(|_| anyhow!("Could not create part"))?;
    let body = multipart::Form::new()
        .text("messaging_product", "whatsapp")
        .part("file", part);
    let res = client
        .post("https://graph.facebook.com/v22.0/609615835573805/media")
        .bearer_auth(token)
        .multipart(body)
        .send()
        .await
        .map_err(|_| anyhow!("Could not upload image."))?;

    let media_upload_response: MediaUploadResponse = res
        .json()
        .await
        .map_err(|_| anyhow!("Could not deserialize media upload response."))?;
    println!("{}", media_upload_response.id);
    Ok(media_upload_response.id)
}
async fn post_razorpay(
    ctx: Extension<ApiContext>,
    axum::extract::Json(payload): axum::extract::Json<Value>,
) -> Result<String> {
    let razorpay_event: RazorPayWebhookEvent = serde_json::from_value(payload)
        .map_err(|_| Error::unprocessable_entity([("Deserialize", "Invalid Request")]))?;
    let razorpay_order_id = razorpay_event.payload.invoice.entity.order_id;
    let order = sqlx::query!(r#"update "order" set progress='READY', is_paid=true where razorpay_order_id = $1 returning from_number"#, razorpay_order_id).fetch_one(&ctx.db).await?;
    let hash = encrypt_order(razorpay_order_id.clone(), &ctx.cipher).await?;
    println!("generated hash: {}", hash);
    let qrcode = generate_qrcode(hash.clone()).await?;
    let media_id = upload_image(qrcode, hash, &ctx.config.whatsapp_token).await?;
    send_message(
        order.from_number,
        SendMessageType::Image(json!({"id": media_id}).to_string()),
        &ctx.config.whatsapp_token,
    )
    .await?;
    Ok("OK".to_string())
}
