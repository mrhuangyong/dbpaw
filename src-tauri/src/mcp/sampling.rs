use super::types::*;

pub struct SamplingHandler;

impl SamplingHandler {
    pub async fn create_message(
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Err(
            "Sampling requires client support. The client must implement sampling/createMessage."
                .to_string(),
        )
    }
}
