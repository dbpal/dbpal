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
