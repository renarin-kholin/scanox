use crate::http::razorpay::{encrypt_order, generate_qrcode};
use crate::http::types::{
    EventNotificationFb, EventNotificationsContactProfileFb, EventNotificationsDocumentFb,
    EventNotificationsInteractiveFb, EventNotificationsInteractiveReplyPayloadFb, ItemType,
};
use crate::http::{ApiContext, Error, RazorpayItem, Result, UserNumer};
use anyhow::anyhow;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Extension, Router};
use bytes::Bytes;
use demoji::demoji;
use log::{error, info};
use lopdf::Document;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::types::Uuid;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Instant;

pub type OrderId = Uuid;
struct WhatsappState {
    user_order: Mutex<HashMap<UserNumer, OrderId>>,
}
#[derive(Serialize, Deserialize)]
struct ImageMessage {
    id: String,
}
#[derive(Serialize, Deserialize)]
struct TextMessage {
    body: String,
}
impl From<&str> for TextMessage {
    fn from(text: &str) -> Self {
        Self {
            body: text.to_string(),
        }
    }
}
impl TextMessage {
    fn as_string(&self) -> Result<String> {
        Ok(serde_json::to_string(self).map_err(|_| anyhow!("Could not convert to string"))?)
    }
}

type ImageMessageStr = String;
#[derive(Serialize, Deserialize)]
pub struct ItemOrderObjectPaymentsAPI {
    name: String,
    amount: PriceOrderObjectPaymentsAPI,
    quantity: i32,
}
#[derive(Serialize, Deserialize)]
pub struct PriceOrderObjectPaymentsAPI {
    value: i32,
    offset: i32,
}
#[derive(Serialize, Deserialize)]
pub struct OrderObjectPaymentsAPI {
    status: String,
    subtotal: PriceOrderObjectPaymentsAPI,
    tax: PriceOrderObjectPaymentsAPI,
    items: Vec<ItemOrderObjectPaymentsAPI>,
}
impl OrderObjectPaymentsAPI {
    pub fn new(item: RazorpayItem, copies: i16, pages: i16) -> OrderObjectPaymentsAPI {
        let order_item = ItemOrderObjectPaymentsAPI {
            name: item.name,
            quantity: copies as i32,
            amount: PriceOrderObjectPaymentsAPI {
                value: item.price * pages as i32,
                offset: 100,
            },
        };
        OrderObjectPaymentsAPI {
            status: "pending".to_string(),
            items: vec![order_item],
            subtotal: PriceOrderObjectPaymentsAPI {
                value: item.price * (copies * pages) as i32,
                offset: 100,
            },
            tax: PriceOrderObjectPaymentsAPI {
                value: 0,
                offset: 100,
            },
        }
    }
}
pub enum SendMessageType<'a> {
    Text(&'a str),
    Image(ImageMessageStr),
    PlaceOrder(),
    CollectPayment(&'a str, OrderObjectPaymentsAPI),
}
#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "order_progress", rename_all = "UPPERCASE")]
pub enum OrderProgress {
    Started,
    Copies,
    Side,
    Color,
    Payment,
    Ready,
    Done,
}
pub fn router() -> Router {
    let whatsapp_state = Arc::new(WhatsappState {
        user_order: Mutex::new(HashMap::new()),
    });
    Router::new()
        .route("/webhook/whatsapp", get(verify_webhook).post(post_whatsapp))
        .with_state(whatsapp_state)
}

pub async fn verify_webhook(
    ctx: Extension<ApiContext>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<String> {
    let mode = query.get("hub.mode");
    let verify_token = query.get("hub.verify_token");
    let challenge = query.get("hub.challenge");
    if mode.is_some_and(|mode| mode.eq("subscribe"))
        && verify_token.is_some_and(|verify_token| {
            verify_token.eq(&ctx.config.webhook_secret) && challenge.is_some()
        })
    {
        Ok(challenge.unwrap().to_string())
    } else {
        Err(Error::unprocessable_entity([("webhook", "Cannot verify ")]))
    }
}
pub async fn send_message(
    to: String,
    message_type: SendMessageType<'_>,
    token: &str,
) -> Result<String> {
    let client = reqwest::Client::new();
    let mut body: HashMap<&str, String> = HashMap::new();
    body.insert("messaging_product", "whatsapp".to_string());
    body.insert("to", to);
    body.insert("recipient_type", "individual".to_string());
    let flow_interactive = json!({
        "type": "flow",
        "body": {
            "text": "Received a document, Click the button below to place a print job."
        },
        "action": {
            "name": "flow",
            "parameters": {
                "flow_cta": "Place Order",
                "flow_id": "1005784394595435",
                "flow_message_version": 3
            }
        }
    })
    .to_string();
    match message_type {
        SendMessageType::Text(text) => {
            body.insert("type", "text".to_string());
            body.insert("text", text.to_string());
        }
        SendMessageType::Image(image) => {
            body.insert("type", "image".to_string());
            body.insert("image", image);
        }
        SendMessageType::PlaceOrder() => {
            body.insert("type", "interactive".to_string());
            body.insert("interactive", flow_interactive);
        }
        SendMessageType::CollectPayment(reference_id, order_obj) => {
            let order_details_interactive = json!({
                "type": "order_details",
                "body": {
                    "text": "Please verify the order and press pay now to proceed."
                },
                "action": {
                    "name": "review_and_pay",
                    "parameters": {
                        "reference_id": reference_id,
                        "type": "digital-goods",
                        "payment_settings": [
                            {
                                "type": "payment_gateway",
                                "payment_gateway": {
                                    "type": "razorpay",
                                    "configuration_name": "Razorpay"
                                },

                            }
                        ],
                        "currency": "INR",
                        "total_amount": order_obj.subtotal,
                        "order": order_obj

                    }
                }
            })
            .to_string();
            error!("{}", order_details_interactive);
            body.insert("type", "interactive".to_string());
            body.insert("interactive", order_details_interactive);
        }
    }
    log::debug!("Sending message.");
    let res = client
        .post("https://graph.facebook.com/v22.0/609615835573805/messages")
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
    url: String,
}
pub async fn get_document(media_id: &str, token: &str) -> Result<Bytes> {
    let client = reqwest::Client::new();
    let res = client
        .get(format!("https://graph.facebook.com/v22.0/{}", media_id))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| Error::NotFound)?;
    let media_response = res
        .json::<GetMediaResponse>()
        .await
        .map_err(|_| Error::NotFound)?;
    let res_file = client
        .get(media_response.url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| Error::NotFound)?;
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
    copies: i16,
    item: &'a str,
}
struct RazorpayCreds<'a> {
    razorpay_key_id: &'a str,
    razorpay_key_secret: &'a str,
}
#[derive(Deserialize)]
struct RazorPayResponse {
    order_id: String,
    short_url: String,
}
async fn generate_invoice(
    order: InvoiceOrderDetails<'_>,
    creds: RazorpayCreds<'_>,
    token: &str,
) -> Result<RazorPayResponse> {
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
        .await
        .map_err(|_| Error::NotFound)?;
    res.json::<RazorPayResponse>()
        .await
        .map_err(|_| Error::NotFound)
}
async fn handle_document_message(
    ctx: Extension<ApiContext>,
    state: State<Arc<WhatsappState>>,
    document: EventNotificationsDocumentFb,
    from: String,
) -> Result<String> {
    if document.filename.ends_with(".pdf") {
        let order_id = sqlx::query_scalar!(
            r#"insert into "order" (from_number, document_id) values ($1, $2) returning order_id"#,
            from.clone(),
            document.id
        )
        .fetch_one(&ctx.db)
        .await?;
        let mut uo = state.user_order.lock().await;
        uo.insert(from.clone(), order_id);
        send_message(
            from.clone(),
            SendMessageType::PlaceOrder(),
            &ctx.config.whatsapp_token,
        )
        .await
    } else {
        Err(Error::unprocessable_entity([(
            "file type",
            "document uploaded is not pdf.",
        )]))
    }
}
async fn handle_interactive(
    ctx: Extension<ApiContext>,
    state: State<Arc<WhatsappState>>,
    interactive: EventNotificationsInteractiveReplyPayloadFb,
    from: String,
) -> Result<String> {
    let uo = state.user_order.lock().await;
    let copies = interactive.copies.parse::<i16>().unwrap_or(1);
    let is_color = interactive.color == "2";
    let is_both_side = interactive.side == "2";
    if let Some(order_id) = uo.get(&from) {
        let order = sqlx::query!(r#"update "order" set copies = $1, is_both_side = $2, is_color = $3, progress='PAYMENT' where order_id = $4 returning from_number, copies, is_both_side, document_id, is_color, order_id"#,copies, is_both_side, is_color, order_id).fetch_one(&ctx.db).await?;
        let razorpay_item = ctx
            .razorpay_items
            .get(&ItemType::from(is_color, is_both_side))
            .ok_or(anyhow!("Could not find item"))?;
        if let Some(copies) = order.copies {
            let pdf_pages = get_pdf_pages(&order.document_id, &ctx.config.whatsapp_token).await?;
            let order_obj_payment =
                OrderObjectPaymentsAPI::new(razorpay_item.clone(), copies, pdf_pages as i16);

            send_message(
                from,
                SendMessageType::CollectPayment(&order_id.to_string()[0..=30], order_obj_payment),
                &ctx.config.whatsapp_token,
            )
            .await?;
        };
    }
    Ok("Test".to_string())
}

async fn post_whatsapp(
    ctx: Extension<ApiContext>,
    state: State<Arc<WhatsappState>>,
    axum::extract::Json(payload): axum::extract::Json<Value>,
) -> Result<String> {
    let event_notification =
        <EventNotificationFb as Deserialize>::deserialize(payload).map_err(|e| {
            error!("{}", e);
            Error::unprocessable_entity([("deserialize", "could not deserialize")])
        })?;
    if !event_notification.entry.is_empty() && !event_notification.entry[0].changes.is_empty() {
        let en_value = event_notification.entry[0].changes[0].clone().value;
        if let Some(en_value) = en_value {
            if let (Some(messages), Some(contacts)) = (en_value.messages, en_value.contacts) {
                if !messages.is_empty() {
                    let message = messages[0].clone();
                    let mut user_name = contacts[0]
                        .clone()
                        .profile
                        .unwrap_or(EventNotificationsContactProfileFb {
                            name: format!("user{:?}", Instant::now()).to_string(),
                        })
                        .name;
                    user_name = demoji(&user_name);
                    let from_number = message.from;
                    let document = message.document;
                    let message_text = message.text;
                    let interactive = message.interactive;
                    if let Some(document) = document {
                        return handle_document_message(ctx, state, document, from_number).await;
                    } else if let Some(interactive) = interactive {
                        let interactive_reply: EventNotificationsInteractiveReplyPayloadFb =
                            serde_json::from_str(&interactive.nfm_reply.response_json)
                                .map_err(|_| anyhow!("Could not deserialize payload."))?;
                        return handle_interactive(ctx, state, interactive_reply, from_number)
                            .await;
                    }
                }
            } else if let Some(statuses) = en_value.statuses {
                if !statuses.is_empty() {
                    let status = &statuses[0].status;

                    if status == "captured" {
                        let uo = state.user_order.lock().await;
                        if let Some(order_id) = uo.get(&statuses[0].recipient_id) {
                            let order = sqlx::query!(r#"update "order" set progress='READY', is_paid=true where order_id = $1 returning from_number"#, order_id).fetch_one(&ctx.db).await?;
                            let hash =
                                encrypt_order(order_id.clone().to_string(), &ctx.cipher).await?;
                            println!("generated hash: {}", hash);
                            let qrcode = generate_qrcode(hash.clone()).await?;
                            let media_id = crate::http::razorpay::upload_image(
                                qrcode,
                                hash,
                                &ctx.config.whatsapp_token,
                            )
                            .await?;
                            send_message(
                                order.from_number,
                                SendMessageType::Image(json!({"id": media_id}).to_string()),
                                &ctx.config.whatsapp_token,
                            )
                            .await?;
                        }
                    }
                }
            }
        }
    } else {
        return Err(Error::from(anyhow!("Invalid event notification")));
    }
    Ok("Thanks for the request".to_string())
}
