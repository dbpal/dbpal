use futures_util::StreamExt;
use langchain_rust::{
    language_models::llm::LLM,
    llm::{OpenAI, OpenAIConfig},
    schemas::Message,
};

use tauri::ipc::Channel;

use crate::app::{self, ErrorFrontend, TaskManager};

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
    op = op.with_temperature(req.temperature);
    op = op.with_top_p(req.top_p);
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
    req: app::OpenAIReq,
    emit: Channel<String>,
    task_id: String,
    task_manager: tauri::State<'_, TaskManager>,
) -> Result<(), ErrorFrontend> {
    let (token, _guard) = task_manager.add(task_id);
    let fur = async {
        let api = make_openai(&req);
        let mut r = api
            .stream(&[Message::new_human_message(req.msg)])
            .await
            .map_err(ErrorFrontend::from)?;
        while let Some(result) = r.next().await {
            //  https://github.com/Abraxas-365/langchain-rust/issues/140

            if let Err(err) = &result {
                if format!("{:?}", err).contains("Stream ended") {
                    break;
                }
            }
            // match &result {
            //     Ok(_) => {}
            //     Err(err) => match err {
            //         LLMError::OpenAIError(open_aierror) => {
            //             if format!("{:?}", open_aierror).contains("Stream ended") {
            //                 break;
            //             }
            //         }
            //         _ => {}
            //     },
            // }
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
