use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpNotification {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

pub struct NotificationBus {
    sender: broadcast::Sender<McpNotification>,
}

impl NotificationBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        Self { sender }
    }

    pub fn notify(&self, notification: McpNotification) {
        let _ = self.sender.send(notification);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<McpNotification> {
        self.sender.subscribe()
    }

    pub fn notify_tools_changed(&self) {
        self.notify(McpNotification {
            method: "notifications/tools/list_changed".to_string(),
            params: None,
        });
    }

    pub fn notify_resources_changed(&self) {
        self.notify(McpNotification {
            method: "notifications/resources/list_changed".to_string(),
            params: None,
        });
    }

    pub fn notify_prompts_changed(&self) {
        self.notify(McpNotification {
            method: "notifications/prompts/list_changed".to_string(),
            params: None,
        });
    }

    pub fn notify_progress(&self, token: &str, progress: u64, total: u64, message: &str) {
        self.notify(McpNotification {
            method: "notifications/progress".to_string(),
            params: Some(serde_json::json!({
                "progressToken": token,
                "progress": progress,
                "total": total,
                "message": message
            })),
        });
    }
}
