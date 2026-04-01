use base64::Engine;
use futures_util::StreamExt;
use langchain_rust::{
    language_models::llm::LLM,
    llm::{OpenAI, OpenAIConfig},
    schemas::{ImageContent, Message, MessageType},
};

use tauri::ipc::Channel;

use crate::app::{self, ErrorFrontend, OpenAIReq, TaskManager};

// fn make_openai(req: &app::OpenAIReq) -> AgentExecutor<OpenAiToolAgent> {
//     #[cfg(debug_assertions)]
//     log::info!("open ai req:{:?}", req);
//     let mut config = OpenAIConfig::default();
//     config = config.with_api_base(&req.url);
//     config = config.with_api_key(&req.api_key);
//     let mut llm = OpenAI::default();
//     llm = llm.with_config(config);
//     llm = llm.with_model(&req.model);
//     let mut op = langchain_rust::language_models::options::CallOptions::default();
//     if let Some(max_token) = req.max_tokens {
//         if max_token != 0 {
//             op = op.with_max_tokens(max_token);
//         }
//     }

//     op = op.with_temperature(req.temperature);
//     op = op.with_top_p(req.top_p);
//     llm = llm.with_options(op);
//     let duckduckgo_tool = DuckDuckGoSearchResults::default();
//     let agent = OpenAiToolAgentBuilder::new()
//         .tools(&[Arc::new(duckduckgo_tool) as Arc<dyn Tool>])
//         .options(ChainCallOptions::new().with_max_tokens(1000))
//         .build(llm)
//         .unwrap();
//     AgentExecutor::from_agent(agent)
// }

fn make_openai(req: &app::OpenAIReq) -> OpenAI<OpenAIConfig> {
    #[cfg(debug_assertions)]
    log::info!("open ai req:{:?}", req);
    let mut config = OpenAIConfig::default();
    config = config.with_api_base(&req.url);
    config = config.with_api_key(&req.api_key);
    let mut api = OpenAI::default();
    api = api.with_config(config);
    api = api.with_model(&req.model);
    let mut op = langchain_rust::language_models::options::CallOptions::default();
    if let Some(max_token) = req.max_tokens {
        if max_token != 0 {
            op = op.with_max_tokens(max_token);
        }
    }
    if let Some(temperature) = req.temperature {
        op = op.with_temperature(temperature);
    }
    if let Some(top_p) = req.top_p {
        op = op.with_top_p(top_p);
    }
    api = api.with_options(op);
    api
}

#[tauri::command]
pub async fn openai_invoke(req: app::OpenAIReq) -> Result<String, ErrorFrontend> {
    let api = make_openai(&req);
    let r = api.invoke(&req.msg).await.map_err(ErrorFrontend::from)?;
    Ok(r)
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
        let api = make_openai(&req);
        let OpenAIReq { msg, images, .. } = req;
        let mut r = api
            .stream(&[Message {
                content: msg,
                message_type: MessageType::HumanMessage,
                id: None,
                tool_calls: None,
                images: images
                    .into_iter()
                    .map(|x| ImageContent {
                        image_url: x,
                        detail: None,
                    })
                    .collect::<Vec<_>>()
                    .into(),
            }])
            .await
            .map_err(ErrorFrontend::from)?;
        while let Some(result) = r.next().await {
            if let Err(err) = &result {
                if format!("{:?}", err).contains("Stream ended") {
                    break;
                }
            }
            let result = result.map_err(ErrorFrontend::from)?;
            emit.send(result.content)?;
        }
        Ok(())
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

    // 必须加上前缀，告知模型图片类型（如 image/jpeg, image/png）
    let r = format!("data:image/jpeg;base64,{}", base64_str);
    anyhow::Ok(r)
}

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;
    use langchain_rust::{
        language_models::llm::LLM,
        llm::{OpenAI, OpenAIConfig},
        schemas::{ImageContent, Message, MessageType},
    };

    use super::*;

    #[tokio::test]

    async fn test_xxx() {
        let mut config = OpenAIConfig::default();
        config = config.with_api_base("https://api.moonshot.cn/v1");
        config = config.with_api_key("sk-2zegNGK2WhxZ9jToc17JBp260Z5vD5Cdd4zXG4s4WISRyczO");
        let mut api = OpenAI::default();
        api = api.with_config(config);
        api = api.with_model("kimi-latest");

        let image = image_to_base64("/Users/yuyang/Desktop/xxx.png")
            .await
            .unwrap();

        let mut r = api
            .stream(&[Message {
                content: "这种图片是什么意思呢".into(),
                message_type: MessageType::HumanMessage,
                id: None,
                tool_calls: None,
                images: Some(vec![ImageContent {
                    image_url: image,
                    detail: None,
                }]),
            }])
            .await
            .unwrap();

        while let Some(r) = r.next().await {
            print!("{}", r.unwrap().content);
        }
    }

    #[tokio::test]
    async fn test_open_api() {
        let mut config = OpenAIConfig::default();
        config = config.with_api_base("https://api.moonshot.cn/v1");
        config = config.with_api_key("sk-2zegNGK2WhxZ9jToc17JBp260Z5vD5Cdd4zXG4s4WISRyczO");
        let mut api = OpenAI::default();
        api = api.with_config(config);
        api = api.with_model("kimi-latest");
        let mut r = api
            .stream(&[Message::new_human_message("hello")])
            .await
            .unwrap();
        while let Some(r) = r.next().await {
            print!("{}", r.unwrap().content);
        }
    }
}
