//! Minimal fixed-origin Bot API client. Errors deliberately omit URLs (which contain tokens).
use crate::control::{Control, Identity, Reply};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    io::Read,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
#[derive(Deserialize)]
pub struct Update {
    pub update_id: i64,
    message: Option<Message>,
    callback_query: Option<Callback>,
}
#[derive(Deserialize)]
struct User {
    id: i64,
    #[serde(default)]
    is_bot: bool,
}
#[derive(Deserialize)]
struct Chat {
    id: i64,
    #[serde(rename = "type")]
    kind: String,
}
#[derive(Deserialize)]
struct Message {
    from: Option<User>,
    chat: Chat,
    text: Option<String>,
}
#[derive(Deserialize)]
struct Callback {
    id: String,
    from: User,
    message: Option<Message>,
    data: Option<String>,
}
#[derive(Deserialize)]
struct Envelope {
    ok: bool,
    result: Option<Value>,
}
pub struct Telegram {
    client: Client,
    token: String,
}
impl Telegram {
    pub fn new(token: String) -> Result<Self, &'static str> {
        if token.len() < 10
            || token.len() > 256
            || !token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b":_-".contains(&b))
        {
            return Err("invalid Telegram token format");
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(40))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .build()
            .map_err(|_| "HTTP client initialization failed")?;
        Ok(Self { client, token })
    }
    fn call(&self, method: &str, payload: Value) -> Result<Value, &'static str> {
        let response = self
            .client
            .post(format!(
                "https://api.telegram.org/bot{}/{method}",
                self.token
            ))
            .json(&payload)
            .send()
            .map_err(|_| "Telegram network request failed")?;
        if !response.status().is_success() {
            return Err("Telegram returned non-success status");
        }
        let mut bytes = Vec::new();
        response
            .take(1_048_577)
            .read_to_end(&mut bytes)
            .map_err(|_| "Telegram response read failed")?;
        if bytes.len() > 1_048_576 {
            return Err("Telegram response too large");
        }
        let result: Envelope =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid Telegram response")?;
        if !result.ok {
            return Err("Telegram rejected request");
        }
        result.result.ok_or("Telegram response missing result")
    }
    fn send(&self, chat: i64, reply: Reply) -> Result<(), &'static str> {
        let mut payload = json!({"chat_id":chat,"text":reply.text});
        if let Some(data) = reply.approve {
            payload["reply_markup"] = json!({"inline_keyboard":[[{"text":"Confirm simulated restart","callback_data":data}]]});
        }
        self.call("sendMessage", payload)?;
        Ok(())
    }
    pub fn run(&self, control: &mut Control) -> Result<(), &'static str> {
        loop {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "Invalid system clock")?
                .as_secs() as i64;
            let offset = control
                .store
                .poll_offset(now)
                .map_err(|_| "Cannot read update cursor")?;
            let value=self.call("getUpdates",json!({"offset":offset,"timeout":25,"limit":25,"allowed_updates":["message","callback_query"]}))?;
            let updates: Vec<Update> =
                serde_json::from_value(value).map_err(|_| "Invalid Telegram updates")?;
            for update in updates {
                if update.update_id < offset {
                    continue;
                }
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| "Invalid system clock")?
                    .as_secs() as i64;
                if let Some(message) = update.message {
                    if let Some(from) = message.from.filter(|f| !f.is_bot) {
                        let who = Identity {
                            actor: from.id,
                            chat: message.chat.id,
                            private: message.chat.kind == "private",
                        };
                        if let Some(text) = message.text {
                            if let Some(reply) = control.command(who, update.update_id, &text, now)
                            {
                                self.send(who.chat, reply)?;
                            }
                        }
                    }
                } else if let Some(callback) = update.callback_query {
                    if let Some(message) = callback.message {
                        let who = Identity {
                            actor: callback.from.id,
                            chat: message.chat.id,
                            private: message.chat.kind == "private" && !callback.from.is_bot,
                        };
                        if let Some(data) = callback.data {
                            if let Some(reply) = control.callback(who, &data, now) {
                                self.call(
                                    "answerCallbackQuery",
                                    json!({"callback_query_id":callback.id}),
                                )?;
                                self.send(who.chat, reply)?;
                            }
                        }
                    }
                }
                control
                    .store
                    .record_update(
                        update.update_id.checked_add(1).ok_or("Invalid update id")?,
                        now,
                    )
                    .map_err(|_| "Cannot persist update cursor")?;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_token_url_injection() {
        assert!(Telegram::new("123456789:abc/../../evil".into()).is_err());
    }
    #[test]
    fn absent_sender_remains_absent() {
        let update: Update = serde_json::from_str(
            r#"{"update_id":1,"message":{"chat":{"id":42,"type":"private"},"text":"/status"}}"#,
        )
        .unwrap();
        assert!(update.message.unwrap().from.is_none());
    }
}
