
use reqwest::multipart::Part;

use std::sync::Arc;
use anyhow::anyhow;
use axum::{Extension, Router};
use axum::extract::State;
use axum::routing::post;
use qrcode_generator::QrCodeEcc;
use reqwest::multipart;
use rsa::RsaPrivateKey;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::http::{ApiContext, Error, Result, UserNumer};
use crate::http::whatsapp::{send_message, SendMessageType};

pub fn router() -> Router {
    Router::new().route("/webhook/razorpay",post(post_razorpay))
}
#[derive(Deserialize)]
struct RazorPayWebhookEventPayloadInvoiceEntity {
    order_id: String,
    status: String,
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
    event: String,
    payload: RazorPayWebhookEventPayload,
}
async fn generate_qrcode(hash: String) -> Result<Vec<u8>> {
    qrcode_generator::to_png_to_vec(hash, QrCodeEcc::Low, 1024).map_err(|_| Error::NotFound)
}

async fn hash_order(order_id: String,  private_key_str: &str )-> Result<String>{
   // Ok( json!({
   //     "order_id": order_id,
   // }).to_string())
    Ok(order_id)
}
#[derive(Deserialize)]
struct MediaUploadResponse {
    id: String,
}
async fn upload_image(image: Vec<u8>, razorpay_order_id: String, token: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let part = multipart::Part::bytes(image)
        .file_name(format!("{}.png", razorpay_order_id))
        .mime_str("image/png")
        .map_err(|_| anyhow!("Could not create part"))?;
    let mut body = multipart::Form::new()
        .text("messaging_product", "whatsapp")
        .part("file", part);
    let res = client
        .post("https://graph.facebook.com/v22.0/609615835573805/media")
        .bearer_auth(token)
        .multipart(body)
        .send()
        .await.map_err(|_| anyhow!("Could not upload image."))?;


    let media_upload_response: MediaUploadResponse = res.json().await.map_err(|_| anyhow!("Could not deserialize media upload response."))?;
    println!("{}",media_upload_response.id);
    Ok(media_upload_response.id)
}
async fn post_razorpay(ctx: Extension<ApiContext>,  axum::extract::Json(payload): axum::extract::Json<Value>) -> Result<String> {
    let razorpay_event: RazorPayWebhookEvent = serde_json::from_value(payload).map_err(|_| Error::unprocessable_entity([("Deserialize", "Invalid Request")]))?;
    let razorpay_order_id = razorpay_event.payload.invoice.entity.order_id;
    let order = sqlx::query!(r#"update "order" set progress='READY', is_paid=true where razorpay_order_id = $1 returning from_number"#, razorpay_order_id).fetch_one(&ctx.db).await?;
    let hash = hash_order(razorpay_order_id.clone(), &ctx.config.private_key).await?;
    let qrcode = generate_qrcode(hash).await?;
    let media_id = upload_image(qrcode, razorpay_order_id, &ctx.config.whatsapp_token).await?;
    send_message(order.from_number, SendMessageType::IMAGE(json!({"id": media_id}).to_string()), &ctx.config.whatsapp_token).await?;
    Ok("OK".to_string())

}