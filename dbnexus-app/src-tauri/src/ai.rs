use std::sync::Arc;

use base64::Engine;
use futures_util::StreamExt;

use rig::agent::{Agent, MultiTurnStreamItem, StreamingError, Text};
use rig::client::completion::CompletionClient;
use rig::EmptyListError;

use crate::app::{self, ErrorFrontend, OpenAIReq, TaskManager};
use rig::completion::{Message, Prompt, PromptError};
use rig::message::{DocumentSourceKind, Image, UserContent};
use rig::one_or_many::OneOrMany;
use rig::providers::openai::responses_api::streaming::StreamingCompletionResponse;
use rig::providers::openai::responses_api::ResponsesCompletionModel;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};
use rig::{client::Client, providers::openai};
use tauri::ipc::Channel;

impl From<PromptError> for ErrorFrontend {
    fn from(value: PromptError) -> Self {
        Self {
            reason: value.to_string(),
            detail: format!("{:?}", value),
            cancelled: false,
        }
    }
}

fn make_openai(req: &app::OpenAIReq) -> anyhow::Result<Agent<ResponsesCompletionModel>> {
    #[cfg(debug_assertions)]
    log::info!("open ai req:{:?}", req);

    let builder = openai::Client::builder()
        .api_key(&req.api_key)
        .base_url(&req.url);
    let api: Client<openai::OpenAIResponsesExt> = builder.build()?;

    let mut agent_builder = api.agent(&req.model).preamble("U are helpful assistant.");

    if let Some(t) = req.max_tokens {
        agent_builder = agent_builder.max_tokens(t);
    }

    if let Some(t) = req.temperature {
        agent_builder = agent_builder.temperature(t);
    }
    let agent: Agent<ResponsesCompletionModel> = agent_builder.build();
    anyhow::Ok(agent)
}

#[tauri::command]
pub async fn openai_invoke(req: app::OpenAIReq) -> Result<String, ErrorFrontend> {
    let api = make_openai(&req).map_err(ErrorFrontend::from)?;
    let r = api.prompt(&req.msg).await?;
    Ok(r)
}

impl From<StreamingError> for ErrorFrontend {
    fn from(value: StreamingError) -> Self {
        Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
            cancelled: false,
        }
    }
}

impl From<EmptyListError> for ErrorFrontend {
    fn from(value: EmptyListError) -> Self {
        Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
            cancelled: false,
        }
    }
}

async fn openai_stream_xxx(
    req: OpenAIReq,
    emit: Arc<dyn Fn(String) + Sync + Send + 'static>,
) -> Result<(), ErrorFrontend> {
    let agent = make_openai(&req)?;

    let msg = Message::User {
        content: {
            let mut content = vec![UserContent::Text(Text {
                text: req.msg.clone(),
            })];
            content.extend(req.images.iter().map(|x| {
                UserContent::Image(Image {
                    data: DocumentSourceKind::Url(x.clone()),
                    media_type: None,
                    detail: None,
                    additional_params: None,
                })
            }));
            OneOrMany::many(content.into_iter())?
        },
    };
    let mut stream = agent.stream_prompt(msg).await;

    while let Some(chunk) = stream.next().await {
        let chunk: MultiTurnStreamItem<StreamingCompletionResponse> =
            chunk.map_err(ErrorFrontend::from)?;

        match chunk {
            MultiTurnStreamItem::StreamAssistantItem(streamed_assistant_content) => {
                match streamed_assistant_content {
                    StreamedAssistantContent::Text(text) => emit(text.text),
                    StreamedAssistantContent::ToolCall { .. } => {}
                    StreamedAssistantContent::ToolCallDelta { .. } => {}
                    StreamedAssistantContent::Reasoning(..) => {}
                    StreamedAssistantContent::ReasoningDelta { .. } => {}
                    StreamedAssistantContent::Final(..) => {}
                }
            }
            MultiTurnStreamItem::StreamUserItem(streamed_user_content) => {
                match streamed_user_content {
                    rig::streaming::StreamedUserContent::ToolResult { .. } => {}
                }
            }
            MultiTurnStreamItem::FinalResponse(final_response) => {
                emit(final_response.response().to_owned())
            }
            _ => return Err(format!("unahdnled chunk type:{:?}", chunk).into()),
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn openai_stream(
    req: OpenAIReq,
    emit: Channel<String>,
    task_id: String,
    task_manager: tauri::State<'_, TaskManager>,
) -> Result<(), ErrorFrontend> {
    let (token, _guard) = task_manager.add(task_id);
    let fur = async {
        openai_stream_xxx(
            req,
            Arc::new(move |s| {
                emit.send(s).unwrap();
            }),
        )
        .await
    };
    tokio::select! {
        res = fur => {res},
        _ = token.cancelled() => {
            Ok(())
        }
    }
}

// 假设 path 是用户上传图片的本地路径

#[allow(unused)]
async fn image_to_base64(path: &str) -> anyhow::Result<String> {
    let image_data = tokio::fs::read(path).await?;
    let base64_str = base64::engine::general_purpose::STANDARD.encode(image_data);

    let mime = if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else {
        "image/png" // 默认
    };

    // 必须加上前缀，告知模型图片类型（如 image/jpeg, image/png）
    let r = format!("data:{};base64,{}", mime, base64_str);
    anyhow::Ok(r)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]

    async fn test_xxx() {
        let image = image_to_base64("/home/yuyang/Pictures/屏幕截图/屏幕截图_20260402_070833.png")
            .await
            .unwrap();

        openai_stream_xxx(
            OpenAIReq {
                url: "https://api.openai-proxy.org/v1".into(),
                api_key: "sk-egR0EUYn0EzVybl7D9JkUFx2ksnD62sZPQ8d0J0yJgPEiAWt".into(),
                model: "gpt-4.1-mini".into(),
                msg: "this image about?".into(),
                temperature: None,
                max_tokens: None,
                top_p: None,
                images: vec![image],
            },
            Arc::new(|x| print!("{}", x)),
        )
        .await
        .unwrap();
    }
}
