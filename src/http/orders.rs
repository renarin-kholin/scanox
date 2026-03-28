use crate::http::razorpay::{QRPayloadData, decrypt_order};
use crate::http::whatsapp::{OrderId, get_document};
use crate::http::{ApiContext, Error, Result};
use anyhow::anyhow;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Uuid;
use std::str::FromStr;
use std::time::SystemTime;
use log::error;
use time::Duration;

pub fn router() -> Router {
    Router::new()
        .route("/verify_qr", post(verify_qrcode))
        .route("/collect_order/{*wildcard}", get(collect_order))
        .route_layer(axum::middleware::from_fn(auth))
}

async fn auth(
    ctx: Extension<ApiContext>,
    mut req: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    let auth_header = if let Some(auth_header) = auth_header {
        auth_header
    } else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    println!("{}", ctx.config.client_secret);
    if auth_header == format!("Bearer {}", ctx.config.client_secret) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
async fn collect_order(
    ctx: Extension<ApiContext>,
    axum::extract::Path(order_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse> {
    let order_id = Uuid::from_str(&order_id).map_err(|_| {
        anyhow!("Invalid order_id")

    })?;
    let order = sqlx::query!(
        r#"select document_id, is_received, is_paid from "order" where order_id = $1"#,
        order_id.clone()
    )
    .fetch_one(&ctx.db)
    .await?;
    let file = get_document(&order.document_id, &ctx.config.whatsapp_token).await?;
    let _ = sqlx::query!(r#"update "order" set progress='DONE', is_received=true where order_id = $1 returning is_received"#, order_id).fetch_one(&ctx.db).await?;
    Ok((
        [(axum::http::header::CONTENT_TYPE, "application/pdf")],
        file,
    ))
}
#[derive(Serialize, Deserialize)]
pub struct VerifyQRCodeRequestBody {
    qrcode_data: String,
}

#[derive(Serialize, Deserialize)]
pub struct OrderDetails {
    order_id: String,
    copies: i16,
    is_color: bool,
    is_both_side: bool,
}
#[derive(Serialize, Deserialize)]
pub struct VerifyQRCodeResponse {
    info: Option<OrderDetails>,
    verified: bool,
}
#[axum::debug_handler]
async fn verify_qrcode(
    ctx: Extension<ApiContext>,
    axum::extract::Json(payload): axum::extract::Json<VerifyQRCodeRequestBody>,
) -> Result<Json<VerifyQRCodeResponse>> {
    let qr_payload: QRPayloadData = decrypt_order(&ctx.cipher, payload.qrcode_data).await?;
    let order_id = OrderId::parse_str(&qr_payload.order_id)
        .map_err(|_| anyhow!("Could not parse order_id"))?;
    let order = sqlx::query!(r#"select is_received, is_paid, order_id, copies, is_color, is_both_side from "order" where order_id = $1"#,  order_id).fetch_one(&ctx.db).await?;
    let mut verify_response = VerifyQRCodeResponse {
        info: None,
        verified: true,
    };
    let is_expired = SystemTime::now()
        .duration_since(qr_payload.created_at)
        .map_err(|_| anyhow!("could not compare times"))?
        > Duration::DAY;
    let verified = if let (Some(is_received), Some(is_paid)) = (order.is_received, order.is_paid) {
        !is_received && is_paid && !is_expired
    } else {
        false
    };
    verify_response.verified = verified;
    if true {
        verify_response.info = Some(OrderDetails {
            order_id: order.order_id.to_string(),
            copies: order.copies.ok_or(anyhow!("Could not get order details"))?,
            is_color: order
                .is_color
                .ok_or(anyhow!("Could not get order details"))?,
            is_both_side: order
                .is_both_side
                .ok_or(anyhow!("Could not get order details"))?,
        })
    }
    Ok(axum::Json(verify_response))
}
