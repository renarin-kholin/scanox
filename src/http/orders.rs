use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Router};

use crate::http::whatsapp::get_document;
use crate::http::{ApiContext, Result};

pub fn router() -> Router {
    Router::new().route("/document/{*wildcard}", get(get_document_file))
}
async fn get_document_file(
    ctx: Extension<ApiContext>,
    axum::extract::Path(razorpay_order_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse> {
    let order = sqlx::query!(
        r#"select document_id from "order" where razorpay_order_id = $1"#,
        razorpay_order_id
    )
    .fetch_one(&ctx.db)
    .await?;

    let file = get_document(&order.document_id, &ctx.config.whatsapp_token).await?;
    Ok((
        [(axum::http::header::CONTENT_TYPE, "application/pdf")],
        file,
    ))
}
