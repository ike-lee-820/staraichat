use crate::types::{Message, ModelKind};
use anyhow::Result;

pub mod agnes;
pub mod deepseek_v3_0324;
pub mod gpt4_1;
pub mod image_generation;
pub mod llama4_scout;
pub mod video_generation;
pub mod xunfei_spark;

pub async fn request(model: ModelKind, messages: &[Message]) -> Result<String> {
    match model {
        ModelKind::Gpt4_1 => gpt4_1::request(messages).await,
        ModelKind::DeepseekV3_0324 => deepseek_v3_0324::request(messages).await,
        ModelKind::Llama4Scout => llama4_scout::request(messages).await,
        ModelKind::XunfeiSpark => xunfei_spark::request(messages).await,
        ModelKind::Agnes => agnes::request(messages).await,
    }
}
