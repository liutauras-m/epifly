pub mod tenant;

pub use tenant::{TenantContext, TenantClaims, PlanTier};

use rig::completion::message::Message;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationContext {
    pub id: Uuid,
    pub system_prompt: Option<String>,
    pub history: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub role: String,
    pub content: String,
}

impl ConversationContext {
    pub fn new(system_prompt: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            system_prompt,
            history: Vec::new(),
        }
    }

    pub fn push_user(&mut self, content: impl Into<String>) {
        self.history.push(HistoryEntry { role: "user".into(), content: content.into() });
    }

    pub fn push_assistant(&mut self, content: impl Into<String>) {
        self.history.push(HistoryEntry { role: "assistant".into(), content: content.into() });
    }

    pub fn to_rig_messages(&self) -> Vec<Message> {
        self.history.iter().map(|h| {
            if h.role == "user" {
                Message::user(&h.content)
            } else {
                Message::assistant(&h.content)
            }
        }).collect()
    }
}
