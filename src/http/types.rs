use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Formatter;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationsContactProfileFb {
    pub name: String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationContactFb {
    pub wa_id: String,

    pub user_id: Option<String>,

    pub profile: Option<EventNotificationsContactProfileFb>,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationErrorsFb {
    pub code: i32,

    pub title: String,

    pub message: String,
}
/// Nested message and enum types in `EventNotificationErrorsFB`.
pub mod event_notification_errors_fb {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone)]
    pub struct ErrorData {
        pub details: String,
    }
}
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationsButtonFb {
    pub payload: String,

    pub text: String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationsContextFb {
    pub forwarded: bool,

    pub frequently_forwarded: bool,

    pub from: String,

    pub id: String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationsDocumentFb {
    pub caption: Option<String>,

    pub filename: String,

    pub sha256: String,

    pub mime_type: String,

    pub id: String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationsImageFb {
    pub caption: Option<String>,

    pub sha256: String,

    pub id: String,

    pub mime_type: String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationsButtonReplyFb {
    pub id: String,

    pub title: String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationsListReplyFb {
    pub id: String,

    pub title: String,

    pub description: String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationsInteractiveTypeFb {
    pub button_reply: Option<EventNotificationsButtonReplyFb>,

    pub list_reply: Option<EventNotificationsListReplyFb>,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationsInteractiveFb {
    pub r#type: Option<EventNotificationsInteractiveTypeFb>,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationsTextFb {
    pub body: String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationsMessagesFb {
    pub from: String,

    pub id: String,

    pub timestamp: String,

    pub r#type: String,

    pub button: Option<EventNotificationsButtonFb>,

    pub interactive: Option<EventNotificationsInteractiveFb>,

    pub text: Option<EventNotificationsTextFb>,

    pub document: Option<EventNotificationsDocumentFb>,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationValueFb {
    pub contacts: Vec<EventNotificationContactFb>,

    pub messaging_product: String,

    pub messages: Vec<EventNotificationsMessagesFb>,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationChangesFb {
    pub field: String,
    pub value: Option<EventNotificationValueFb>,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationEntryFb {
    pub id: String,
    pub changes: Vec<EventNotificationChangesFb>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct EventNotificationFb {
    pub object: String,
    pub entry: Vec<EventNotificationEntryFb>,
}

#[derive(sqlx::Type)]
pub struct Timestamptz(pub OffsetDateTime);

impl Serialize for Timestamptz {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&self.0.format(&Rfc3339).unwrap())
    }
}
impl<'de> Deserialize<'de> for Timestamptz {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StrVisitor;
        impl Visitor<'_> for StrVisitor {
            type Value = Timestamptz;
            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.pad("expected string")
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                OffsetDateTime::parse(v, &Rfc3339)
                    .map(Timestamptz)
                    .map_err(E::custom)
            }
        }
        deserializer.deserialize_str(StrVisitor)
    }
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum ItemType {
    BW,
    BWT,
    C,
    CT,
}

impl ItemType {
    pub fn from(color: bool, both_side: bool) -> Self {
        match (color, both_side) {
            (false, false) => Self::BW,
            (false, true) => Self::BWT,
            (true, false) => Self::C,
            (true, true) => Self::CT,
        }
    }
}
