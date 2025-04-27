use std::collections::HashMap;
use std::sync::{Arc};
use anyhow::anyhow;
use axum::extract::{Query, State};
use axum::{Extension, Router};
use axum::routing::get;
use bytes::Bytes;
use demoji::demoji;
use lopdf::Document;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::types::Uuid;
use tokio::sync::Mutex;
use tokio::time::Instant;
use crate::http::{ApiContext, Error, Result, UserNumer};
use crate::http::types::{EventNotificationFb, EventNotificationsContactProfileFb, EventNotificationsDocumentFb, ItemType};

pub type OrderId = Uuid;
struct WhatsappState {
    user_order: Mutex<HashMap<UserNumer, OrderId>>
}
#[derive(Serialize, Deserialize)]
struct ImageMessage {
    id: String
}
#[derive(Serialize, Deserialize)]
struct TextMessage {
    body: String
}
impl From<&str> for TextMessage {
     fn from(text: &str) -> Self {
        Self {body: text.to_string()}
    }
}
impl TextMessage {
    fn as_string(&self) -> Result<String> {
        Ok(serde_json::to_string(self).map_err(|_| anyhow!("Could not convert to string"))?)
    }
}

type ImageMessageStr = String;
pub enum SendMessageType<'a> {
    TEXT(&'a str),
    IMAGE(ImageMessageStr)
}
#[derive(Debug, sqlx::Type)]
#[sqlx(type_name="order_progress", rename_all="UPPERCASE")]
pub enum OrderProgress {
    Started,
    Copies,
    Side,
    Color,
    Payment,
    Ready,
    Done

}
pub fn router() -> Router {
    let whatsapp_state = Arc::new(WhatsappState {
        user_order: Mutex::new(HashMap::new())
    });
    Router::new()
        .route("/webhook/whatsapp", get(verify_webhook).post(post_whatsapp)).with_state(whatsapp_state)
}


pub async fn verify_webhook(ctx: Extension<ApiContext>, Query(query): Query<HashMap<String, String>>) -> Result<String> {
    let mode = query.get("hub.mode");
    let verify_token = query.get("hub.verify_token");
    let challenge = query.get("hub.challenge");
    if mode.is_some_and(|mode| mode.eq("subscribe"))
        && verify_token.is_some_and(|verify_token| verify_token.eq(&ctx.config.webhook_secret) && challenge.is_some())
    {
        Ok(challenge.unwrap().to_string())
    } else {
       Err(Error::unprocessable_entity([("webhook","Cannot verify ")]))
    }
}

pub async fn send_message(to: String, message_type: SendMessageType<'_>, token: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let mut body = HashMap::new();
    body.insert("messaging_product", "whatsapp");
    body.insert("to", &to);
    body.insert("recipient_type", "individual");
    match message_type {
        SendMessageType::TEXT( text) => {
            body.insert("type", "text");
            body.insert("text", text);
        }
        SendMessageType::IMAGE(ref image) => {
            body.insert("type", "image");
            body.insert("image", image);
        }
    }
    log::debug!("Sending message.");
    let res = client.post("https://graph.facebook.com/v22.0/609615835573805/messages")
        .json(&body)
        .bearer_auth(token)
        .send()
        .await;
    let res_ = res.map_err(|err| {
        log::error!("Reqwest error {}", err);
        println!("{:?}", err);
       Error::unprocessable_entity([("Error", "Error sending message")])
    })?;
    println!("{:?}", res_);
    Ok("Message sent.".to_string())
}
#[derive(Deserialize)]
struct GetMediaResponse {
    url: String
}
pub async fn get_document(media_id: &str, token: &str) -> Result<Bytes> {
    let client = reqwest::Client::new();
    let res = client
        .get(format!("https://graph.facebook.com/v22.0/{}", media_id))
        .bearer_auth(token)
        .send()
        .await.map_err(|_| {
        Error::NotFound
    })?;
    let media_response = res.json::<GetMediaResponse>().await.map_err(|_| {
        Error::NotFound
    })?;
    let res_file = client
        .get(media_response.url)
        .bearer_auth(token)
        .send()
        .await.map_err(|_| Error::NotFound)?;
     res_file.bytes().await.map_err(|_| Error::NotFound)


}
async fn get_pdf_pages(media_id: &str, token: &str) -> Result<u8> {
    let file = get_document(media_id, token).await?;
    let pdf_file = Document::load_mem(&file[..]).map_err(|_| Error::NotFound)?;
    Ok(pdf_file.page_iter().count() as u8)


}
struct InvoiceOrderDetails<'a> {
    order_id: &'a str,
    document_id: &'a str,
    from_number: &'a str,
    user_name: &'a str,
    is_both_side: bool,
    copies: i16,
    is_color: bool,
    item: &'a str,

}
struct RazorpayCreds<'a> {
    razorpay_key_id: &'a str,
    razorpay_key_secret: &'a str
}
#[derive(Deserialize)]
struct RazorPayResponse {
    order_id: String,
    short_url: String
}
async fn generate_invoice( order: InvoiceOrderDetails<'_>, creds: RazorpayCreds<'_>, token: &str) -> Result<RazorPayResponse> {
    let client = reqwest::Client::new();
    let description = format!("Payment invoice for scanox order id {}", order.order_id);
    let customer = json!({
        "name": order.user_name,
        "contact": order.from_number
    });
    let page_number = get_pdf_pages(order.document_id, token).await?;


    let body = json!({
        "type": "invoice",
        "description": description,
        "customer": customer,
        "line_items": [{"item_id": order.item, "quantity": order.copies * page_number as i16}]
    });
    let res = client
        .post("https://api.razorpay.com/v1/invoices")
        .json(&body)
        .basic_auth(creds.razorpay_key_id, Some(creds.razorpay_key_secret))
        .send()
        .await.map_err(|_| Error::NotFound)?;
     res.json::<RazorPayResponse>().await.map_err(|_| Error::NotFound)


}
async fn handle_document_message(ctx: Extension<ApiContext>,state: State<Arc<WhatsappState>>, document: EventNotificationsDocumentFb, from: String) -> Result<String> {
    if document.filename.ends_with(".pdf") {

        let order_id = sqlx::query_scalar!(
            r#"insert into "order" (from_number, document_id) values ($1, $2) returning order_id"#,
            from.clone(),
            document.id
        ).fetch_one(&ctx.db).await?;
        let mut uo = state.user_order.lock().await;
        uo.insert(from.clone(), order_id);
         send_message(from.clone(), SendMessageType::TEXT(&TextMessage::from("Starting a new print order, Please enter the number of copies you want.").as_string()?), &ctx.config.whatsapp_token).await

    } else {
         Err(Error::unprocessable_entity([("file type", "document uploaded is not pdf.")]))
    }

}


async fn handle_text_message(ctx: Extension<ApiContext>,state: State<Arc<WhatsappState>>,text: String, user_name: String, from: String) -> Result<String> {
    let  uo = state.user_order.lock().await;
    if let Some(order_id) = uo.get(&from) {
        let order = sqlx::query!(r#" select progress as "progress!: OrderProgress" from "order" where order_id=$1"#, order_id).fetch_optional(&ctx.db).await?.ok_or(Error::unprocessable_entity([("order", "order does not exist")]))?;
        match order.progress {
            OrderProgress::Started => {
                let copies = text.parse::<u16>();
                if copies.is_err() {
                    send_message(from.clone(), SendMessageType::TEXT(&TextMessage::from("That is not a valid number. Proceeding with 1 copies.").as_string()?), &ctx.config.whatsapp_token).await?;
                }
                if let Ok(copies) = copies {
                    sqlx::query_scalar!(r#"update "order" set copies = $1, progress='COPIES' where order_id = $2"#, copies as i32, order_id).fetch_optional(&ctx.db).await?;
                    send_message(from, SendMessageType::TEXT(&TextMessage::from("Do you want both side print? Answer Yes or No").as_string()?), &ctx.config.whatsapp_token).await?;
                }
            }
            OrderProgress::Copies => {
                let mut is_both_side = false;
                if text.to_lowercase().contains("yes") {
                    is_both_side = true;
                }
                let order = sqlx::query!(r#"update "order" set is_both_side = $1, progress='PAYMENT' where order_id = $2 returning from_number, copies, is_both_side, document_id, is_color, order_id"#, is_both_side, order_id).fetch_one(&ctx.db).await?;
                send_message(from.clone(), SendMessageType::TEXT(&TextMessage::from("Generating payment link, please wait.").as_string()?), &ctx.config.whatsapp_token).await?;
                if let(Some(is_color), Some(is_both_side), Some(copies)) = (order.is_color, order.is_both_side, order.copies) {
                    let item_type = ItemType::from(is_color, is_both_side);
                    let item = ctx.razorpay_items.get(&item_type).ok_or(Error::NotFound)?;
                    let razorpay_res = generate_invoice(InvoiceOrderDetails {
                        order_id: &order.order_id.to_string(),
                        item,
                        is_color,
                        is_both_side,
                        document_id: &order.document_id,
                        from_number: &order.from_number,
                        copies,
                        user_name: &user_name
                    }, RazorpayCreds{
                        razorpay_key_id: &ctx.config.razorpay_key_id,
                        razorpay_key_secret: &ctx.config.razorpay_key_secret
                    }, &ctx.config.whatsapp_token).await?;
                    sqlx::query!(r#"update "order" set razorpay_order_id = $1 where order_id = $2"#, razorpay_res.order_id, order_id).fetch_optional(&ctx.db).await?;
                    send_message(from, SendMessageType::TEXT(&TextMessage::from(razorpay_res.short_url.as_str()).as_string()?), &ctx.config.whatsapp_token).await?;

                }


            }
            OrderProgress::Side => {}
            OrderProgress::Color => {}
            OrderProgress::Payment => {}
            OrderProgress::Ready => {}
            OrderProgress::Done => {}
        }
        Ok("Test".to_string())
    } else {
        Err(Error::NotFound)
    }
}

async fn post_whatsapp(ctx: Extension<ApiContext>, state: State<Arc<WhatsappState>>, axum::extract::Json(payload): axum::extract::Json<Value>) -> Result<String> {
    let event_notification = <EventNotificationFb as Deserialize>::deserialize(payload).map_err(|_| {
        Error::unprocessable_entity([("deserialize", "could not deserialize")])
    })?;
    if !event_notification.entry.is_empty() && !event_notification.entry[0].changes.is_empty() {
        let en_value = event_notification.entry[0].changes[0].clone().value;
        if let Some(en_value) = en_value {
            if !en_value.messages.is_empty() {
                let message = en_value.messages[0].clone();
                let mut user_name = en_value.contacts[0].clone().profile.unwrap_or(
                    EventNotificationsContactProfileFb {
                        name: format!("user{:?}", Instant::now()).to_string()
                    }).name;
                user_name = demoji(&user_name);
                let from_number = message.from;
                let document = message.document;
                let message_text = message.text;
                if let Some(document) = document {
                    return handle_document_message(ctx, state, document, from_number).await;
                } else if let Some(text) = message_text {
                    return handle_text_message(ctx,state, text.body, user_name, from_number).await;
                }
            }
        }
    } else {
        return Err(Error::from(anyhow!("Invalid event notification")));
    }
    Ok("Thanks for the request".to_string())
}