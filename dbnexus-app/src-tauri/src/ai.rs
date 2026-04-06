use base64::Engine;
use dbnexus_db::db::{BackendKind, ExportFrame, ExportKind};
use futures_util::StreamExt;
use rig::agent::{Agent, MultiTurnStreamItem, PromptHook, StreamingError, Text};
use rig::client::completion::CompletionClient;
use rig::providers::openai::responses_api::ResponsesCompletionModel;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};
use rig::tool::ToolDyn;
use rig::wasm_compat::{WasmCompatSend, WasmCompatSync};
use rig::EmptyListError;
use std::future::Future;
use std::sync::Arc;

use crate::app::{
    self, AiOnlineSearch, DBPal, DatabaseBackend, ErrorFrontend, OpenAIReq, TaskManager,
};
use rig::completion::{CompletionModel, Message, Prompt, PromptError, ToolDefinition};
use rig::message::{DocumentSourceKind, Image, ReasoningContent, ToolResultContent, UserContent};
use rig::one_or_many::OneOrMany;
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

fn make_openai(
    req: &app::OpenAIReq,
    tools: Vec<Box<dyn ToolDyn>>,
) -> anyhow::Result<Agent<ResponsesCompletionModel>> {
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
    let agent = agent_builder.tools(tools).build();
    anyhow::Ok(agent)
}

#[tauri::command]
pub async fn openai_invoke(req: app::OpenAIReq) -> Result<String, ErrorFrontend> {
    let api = make_openai(&req, vec![]).map_err(ErrorFrontend::from)?;
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

async fn call_openai_stream(
    req: &OpenAIReq,
    tools: Vec<Box<dyn ToolDyn>>,
    emit: Emitter,
    comment_fn: impl Fn(&str) -> String + Send + Sync + 'static,
) -> Result<(), ErrorFrontend> {
    let agent = make_openai(req, tools)?;
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
    let mut prompt = agent.stream_prompt(msg).await;
    while let Some(chunk) = prompt.next().await {
        let chunk = chunk?;
        match chunk {
            MultiTurnStreamItem::StreamAssistantItem(item) => match item {
                rig::streaming::StreamedAssistantContent::Text(text) => {
                    emit(text.text().to_owned());
                }
                StreamedAssistantContent::ToolCall { .. } => {
                    // tool call have notice
                }
                StreamedAssistantContent::ToolCallDelta { .. } => {}
                StreamedAssistantContent::Reasoning(reasoning) => {
                    reasoning.content.iter().for_each(|x| match x {
                        ReasoningContent::Text { text, signature: _ } => {
                            emit(text.clone());
                        }
                        ReasoningContent::Encrypted(_) => {}
                        ReasoningContent::Redacted { .. } => {}
                        ReasoningContent::Summary(summary) => emit(summary.clone()),
                        _ => {
                            log::error!("maybe handle type:{:?}", x);
                        }
                    })
                }
                StreamedAssistantContent::ReasoningDelta { reasoning, .. } => emit(reasoning),
                StreamedAssistantContent::Final(x) => {
                    emit(format!(
                        "\ninput token:{} output token:{} total token:{}",
                        x.usage.input_tokens, x.usage.output_tokens, x.usage.total_tokens,
                    ));
                }
            },
            MultiTurnStreamItem::StreamUserItem(x) => match x {
                rig::streaming::StreamedUserContent::ToolResult {
                    tool_result,
                    internal_call_id: _,
                } => {
                    // those text in
                    for x in tool_result.content.iter() {
                        match x {
                            ToolResultContent::Text(text) => emit(comment_fn(text.text())),
                            ToolResultContent::Image(..) => {
                                return Err(ErrorFrontend::from(
                                    "we should not support generate image???",
                                ))
                            }
                        }
                    }
                }
            },
            MultiTurnStreamItem::FinalResponse(..) => {
                // emit(final_response.response().to_owned())
            }
            _ => {
                return Err(ErrorFrontend {
                    reason: format!("unhandled stream item: {:?}", chunk),
                    detail: "".into(),
                    cancelled: false,
                })
            }
        }
    }

    Ok(())
}

#[derive(Clone)]
#[allow(unused)]
pub struct EmitSomeStatus {
    emit: Emitter,
}
impl<M> PromptHook<M> for EmitSomeStatus where M: CompletionModel + 'static {}

impl DatabaseBackend {
    fn make_tools(&self, query_id: impl Into<String>, emit: Emitter) -> Vec<Box<dyn ToolDyn>> {
        match self.backend_kind() {
            BackendKind::Mongo => {
                vec![]
            }

            _ => {
                // sql base database
                vec![Box::new(SqlQueryTool {
                    db: Clone::clone(self),
                    query_id: query_id.into(),
                    emit,
                })]
            }
        }
    }
}

pub struct SqlQueryTool {
    db: DatabaseBackend,
    query_id: String,
    emit: Emitter,
}

#[derive(Clone, Debug)]
pub struct CallToolErr {
    err_msg: String,
}
impl std::fmt::Display for CallToolErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.err_msg)
    }
}

impl<T: Into<String>> From<T> for CallToolErr {
    fn from(value: T) -> Self {
        Self {
            err_msg: value.into(),
        }
    }
}

impl std::error::Error for CallToolErr {}

use rig::tool::Tool;

#[derive(serde::Deserialize, Debug)]

pub struct SqlQueryArgs {
    query: String,
}

#[derive(Debug, serde:: Serialize)]
pub struct SqlQueryOutput {
    result: serde_json::Value,
    query: String,
}

pub type Emitter = Arc<dyn Fn(String) + Send + Sync + 'static>;

impl Tool for SqlQueryTool {
    const NAME: &'static str = "sql_query";

    type Args = SqlQueryArgs;
    type Output = SqlQueryOutput;
    type Error = CallToolErr;

    #[allow(clippy::manual_async_fn)]
    fn definition(
        &self,
        _prompt: String,
    ) -> impl Future<Output = ToolDefinition> + WasmCompatSend + WasmCompatSync {
        async move {
            ToolDefinition {
                name: Self::NAME.to_string(),
                description: "执行 SQL 查询并返回结果。只支持 SELECT 只读查询。".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "要执行的 SQL SELECT 查询语句"
                        }
                    },
                    "required": ["query"]
                }),
            }
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        (self.emit)(
            self.db
                .backend_kind()
                .comment(format!("call tool sql query {}", args.query).as_str()),
        );
        let (exceed, ret) = self
            .db
            .as_export_query_result()
            .export_query_result_collect(
                Some(&self.query_id),
                &args.query,
                ExportKind::JsonLine,
                100,
            )
            .await
            .map_err(|e| CallToolErr::from(format!("查询失败: {}", e)))?;
        if exceed {
            (self.emit)("-- data exceed 100,some data not feed to LLM".to_string())
        }
        let r = expect_export_frame_all_json(ret)?;
        Ok(SqlQueryOutput {
            result: serde_json::json!({"data": r}),
            query: args.query,
        })
    }
}

fn expect_export_frame_all_json(
    x: Vec<ExportFrame>,
) -> Result<Vec<serde_json::Value>, CallToolErr> {
    let mut ret = vec![];
    for x in x {
        match x {
            ExportFrame::JsonValue(x) => {
                ret.push(x);
            }
            _ => {
                return Err(CallToolErr {
                    err_msg: format!("expect json line but:{}", x),
                });
            }
        }
    }
    Ok(ret)
}

#[tauri::command]
pub async fn openai_stream(
    req: OpenAIReq,
    name: String,
    query_id: String,
    emit: Channel<String>,
    task_id: String,
    task_manager: tauri::State<'_, TaskManager>,
    dbpal: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let (token, _guard) = task_manager.add(task_id);
    let db = dbpal.get_backend(&name).await?;
    let emitter = Arc::new(move |s| {
        if let Err(err) = emit.send(s) {
            log::error!("Error sending message: {:?}", err)
        }
    });
    let mut tools = db.make_tools(query_id, emitter.clone());
    if let Some(x) = req.search_engine.make_tool(emitter.clone())? {
        tools.push(x);
    }
    let db_kind = db.backend_kind();
    let fur = call_openai_stream(&req, tools, emitter, move |x| db_kind.comment(x));
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

impl AiOnlineSearch {
    fn make_tool(&self, emit: Emitter) -> Result<Option<Box<dyn ToolDyn>>, ErrorFrontend> {
        if self.enable {
            let engine: Box<dyn websearch::SearchProvider> = match self.provider.as_ref() {
                "duckduckgo" => Box::new(websearch::providers::DuckDuckGoProvider::new()),
                _ => {
                    return Err(ErrorFrontend::from(format!(
                        "search engine found:{}",
                        self.provider
                    )));
                }
            };
            Ok(Some(Box::new(WebSearchTool { engine, emit })))
        } else {
            Ok(None)
        }
    }
}

/// 单个搜索结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchInput {
    /// 搜索查询字符串
    pub query: String,
    pub max_results: Option<u32>,
}

/// 搜索工具的输出结果
#[derive(Debug, Clone, serde:: Serialize, serde::Deserialize)]
pub struct SearchOutput {
    /// 搜索查询
    pub query: String,
    /// 搜索结果列表
    pub results: Vec<websearch::SearchResult>,
    /// 搜索耗时 (毫秒)
    pub search_time_ms: u64,
}

pub struct WebSearchTool {
    engine: Box<dyn websearch::SearchProvider>,
    emit: Emitter,
}

impl Tool for WebSearchTool {
    const NAME: &'static str = "web_search";

    type Error = CallToolErr;

    type Args = SearchInput;

    type Output = SearchOutput;

    #[allow(clippy::manual_async_fn)]
    fn definition(
        &self,
        _prompt: String,
    ) -> impl Future<Output = ToolDefinition> + WasmCompatSend + WasmCompatSync {
        async move {
            ToolDefinition {
                name: Self::NAME.to_string(),
                description: "Search the web for information and use the results to answer the user's question. \
                    Use this tool when you need to find current information, facts, news, or specific resources \
                    that may not be in your training data. The tool returns relevant web pages with titles, \
                    URLs, and snippets."
                    .to_string(),
                parameters :serde_json::json!({
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "The search query string - what to search for"
                            },
                            "max_results": {
                                "type": "integer",
                                "description": "Maximum number of results to return (default: 10, max: 20)",
                                "minimum": 10,
                                "maximum": 20,
                                "nullable": true
                            }
                        },
                        "required": ["query"]
                    })
            }
        }
    }
    #[allow(clippy::manual_async_fn)]
    fn call(
        &self,
        args: Self::Args,
    ) -> impl Future<Output = Result<Self::Output, Self::Error>> + WasmCompatSend {
        async move {
            (self.emit)(format!(
                "-- search internet for answer,query:{}\n",
                args.query
            ));

            let start = std::time::Instant::now();
            let results = self
                .engine
                .search(&websearch::SearchOptions {
                    query: args.query.clone(),
                    max_results: args.max_results,
                    ..Default::default()
                })
                .await
                .map_err(|err| {
                    CallToolErr::from(format!("call search engine failed,err:{}", err))
                })?;

            #[cfg(debug_assertions)]
            {
                for r in results.iter() {
                    println!("search result-{}-{:?} ", r.title, r.snippet);
                }
            }

            (self.emit)(format!(
                "-- search done:{} results {} ...\n",
                results.len(),
                {
                    //
                    let slice = results.get(0..3).unwrap_or(&results);
                    let r = slice
                        .iter()
                        .map(|x| x.snippet.clone().unwrap_or_default())
                        .collect::<Vec<_>>()
                        .join("  ");

                    if r.len() > 100 {
                        r.chars().take(100).collect::<String>()
                    } else {
                        r
                    }
                }
            ));

            Ok(SearchOutput {
                query: args.query.clone(),
                search_time_ms: start.elapsed().as_millis() as u64,
                results,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbnexus_db::{datasource_config::SqliteDBConfig, db::QuerySql, sqlite::SqliteDB};
    use futures_util::StreamExt;
    use rig::{
        agent::MultiTurnStreamItem,
        streaming::{StreamedAssistantContent, StreamedUserContent, StreamingPrompt},
    };
    use websearch::SearchProvider;

    #[tokio::test]

    async fn test_xxx() {
        call_openai_stream(
            &OpenAIReq {
                url: "https://api.openai-proxy.org/v1".into(),
                api_key: "sk-egR0EUYn0EzVybl7D9JkUFx2ksnD62sZPQ8d0J0yJgPEiAWt".into(),
                model: "gpt-4.1-mini".into(),
                msg: "hello can u help me".into(),
                temperature: None,
                max_tokens: None,
                top_p: None,
                images: vec![],
                search_engine: Default::default(),
            },
            vec![],
            Arc::new(|x| print!("{}", x)),
            |x| x.to_owned(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]

    async fn test_xxx_qwen() {
        call_openai_stream(
            &OpenAIReq {
                url: "https://dashscope.aliyuncs.com/compatible-mode/v1".into(),
                api_key: "sk-6c19c965d3234c8fae49c19e4f0301f0".into(),
                model: "qwen3.5-flash".into(),
                msg: "hello can u help me".into(),
                temperature: None,
                max_tokens: None,
                top_p: None,
                images: vec![],
                search_engine: Default::default(),
            },
            vec![],
            Arc::new(|x| print!("{}", x)),
            |x| x.to_owned(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]

    async fn test_fine_with_tool() {
        // enable_test_log();

        let db = SqliteDB::new(SqliteDBConfig {
            path: "/home/yuyang/tests/sqlite.test.db".into(),
            read_only: true,
            conn_timeout: None,
        })
        .await
        .unwrap();
        let req =  &OpenAIReq {
                url: "https://api.openai-proxy.org/v1".into(),
                api_key: "sk-egR0EUYn0EzVybl7D9JkUFx2ksnD62sZPQ8d0J0yJgPEiAWt".into(),
                model: "gpt-4.1-mini".into(),
                msg: r#"


system:You are a sqlite database expert.
Your answer must follow ALL of these rules:
1. Comments must use "--" syntax.
2. system locale is zh-CN 


human: here are some information about the database
==============================database sqlitexxx==============================
we have fellow tables: Persons,departments,employees,has_data_0,has_data_1,sqlite_sequence,test_column,test_commit,test_create_trigger,test_create_trigger_update,test_duplicate,test_duplicate2,test_index,test_rename2,test_table,test_table_pagination
we have fellow view: test_view,employees_view
Persons is table
        Columns:ID,Name,Age
        Primary key: ID
Persons max id is 3
Persons create table sql is:
CREATE TABLE Persons (
                ID INTEGER PRIMARY KEY,
                Name TEXT,
                Age INTEGER
            );
CREATE INDEX persons_idx_age ON Persons (Age);



Chat History:



human: tell me Mike's age from TABLE Persons 
                "#.into(),
                temperature: None,
                max_tokens: None,
                top_p: None,
                images: vec![],
                search_engine : Default::default(),
            };
        let query_id = "aaaa";

        db.open_connection(query_id).await.unwrap();

        log::info!("open ai req:{:?}", req);

        let builder = openai::Client::builder()
            .api_key(&req.api_key)
            .base_url(&req.url);
        let api: Client<openai::OpenAIResponsesExt> = builder.build().unwrap();

        let mut agent_builder = api.agent(&req.model).preamble("U are helpful assistant.");

        if let Some(t) = req.max_tokens {
            agent_builder = agent_builder.max_tokens(t);
        }

        if let Some(t) = req.temperature {
            agent_builder = agent_builder.temperature(t);
        }

        let agent_builder = agent_builder.tool(SqlQueryTool {
            db: DatabaseBackend::Schema(Arc::new(db)),
            query_id: query_id.to_owned(),
            emit: Arc::new(|_| {}),
        });

        let agent = agent_builder.build();

        let r = agent.prompt(&req.msg).await.unwrap();

        println!("{}", r);
    }

    #[tokio::test]

    async fn test_steam_not_ok() {
        let db = SqliteDB::new(SqliteDBConfig {
            path: "/home/yuyang/tests/sqlite.test.db".into(),
            read_only: true,
            conn_timeout: None,
        })
        .await
        .unwrap();
        let req =  &OpenAIReq {
                url: "https://api.openai-proxy.org/v1".into(),
                api_key: "sk-egR0EUYn0EzVybl7D9JkUFx2ksnD62sZPQ8d0J0yJgPEiAWt".into(),
                model: "gpt-4.1-mini".into(),
                msg: r#"


system:You are a sqlite database expert.
Your answer must follow ALL of these rules:
1. Comments must use "--" syntax.
2. system locale is zh-CN 


human: here are some information about the database
==============================database sqlitexxx==============================
we have fellow tables: Persons,departments,employees,has_data_0,has_data_1,sqlite_sequence,test_column,test_commit,test_create_trigger,test_create_trigger_update,test_duplicate,test_duplicate2,test_index,test_rename2,test_table,test_table_pagination
we have fellow view: test_view,employees_view
Persons is table
        Columns:ID,Name,Age
        Primary key: ID
Persons max id is 3
Persons create table sql is:
CREATE TABLE Persons (
                ID INTEGER PRIMARY KEY,
                Name TEXT,
                Age INTEGER
            );
CREATE INDEX persons_idx_age ON Persons (Age);



Chat History:



human: tell me Mike's age from TABLE Persons 
                "#.into(),
                temperature: None,
                max_tokens: None,
                top_p: None,
                images: vec![],
                search_engine: Default::default(),
            };
        let query_id = "aaaa";

        db.open_connection(query_id).await.unwrap();

        #[cfg(debug_assertions)]
        log::info!("open ai req:{:?}", req);

        let builder = openai::Client::builder()
            .api_key(&req.api_key)
            .base_url(&req.url);
        let api: Client<openai::OpenAIResponsesExt> = builder.build().unwrap();

        let mut agent_builder = api.agent(&req.model).preamble("U are helpful assistant.");

        if let Some(t) = req.max_tokens {
            agent_builder = agent_builder.max_tokens(t);
        }

        if let Some(t) = req.temperature {
            agent_builder = agent_builder.temperature(t);
        }

        let agent = agent_builder
            .tool(SqlQueryTool {
                db: DatabaseBackend::Schema(Arc::new(db)),
                query_id: query_id.to_owned(),
                emit: Arc::new(|_| {}),
            })
            .build();
        let mut stream = agent.stream_prompt(req.msg.clone()).await;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(ErrorFrontend::from).unwrap();

            match chunk {
                MultiTurnStreamItem::StreamAssistantItem(streamed_assistant_content) => {
                    match streamed_assistant_content {
                        StreamedAssistantContent::Text(text) => print!("{}", text.text),
                        StreamedAssistantContent::ToolCall {
                            tool_call,
                            internal_call_id,
                        } => {
                            println!("toolcall:{:?} {}", tool_call, internal_call_id);
                        }
                        StreamedAssistantContent::ToolCallDelta { .. } => {}
                        StreamedAssistantContent::Reasoning(..) => {}
                        StreamedAssistantContent::ReasoningDelta { .. } => {}
                        StreamedAssistantContent::Final(..) => {}
                    }
                }
                MultiTurnStreamItem::StreamUserItem(streamed_user_content) => {
                    match streamed_user_content {
                        StreamedUserContent::ToolResult { .. } => {}
                    }
                }
                MultiTurnStreamItem::FinalResponse(final_response) => {
                    print!("{}", final_response.response().to_owned())
                }
                _ => unreachable!(),
            }
        }

        println!("done");
    }

    use std::env;

    #[tokio::test]
    async fn test_search_with_proxy() {
        // enable_test_log();

        // 打印当前代理设置
        println!("Current proxy settings:");
        println!("  http_proxy:  {:?}", env::var("http_proxy").ok());
        println!("  https_proxy: {:?}", env::var("https_proxy").ok());
        println!("  all_proxy:   {:?}", env::var("all_proxy").ok());

        // 如果没有设置代理，手动设置（仅用于测试）
        if env::var("https_proxy").is_err() {
            println!("⚠️  No proxy detected, setting default proxy...");
            env::set_var("http_proxy", "http://127.0.0.1:7897");
            env::set_var("https_proxy", "http://127.0.0.1:7897");
            env::set_var("all_proxy", "socks5://127.0.0.1:7897");
        }

        let provider = websearch::providers::DuckDuckGoProvider::new();

        match provider
            .search(&websearch::SearchOptions {
                query: "rust language".into(),
                max_results: 10.into(),
                ..Default::default()
            })
            .await
        {
            Ok(results) => {
                println!("✅ Search successful, found {} results:", results.len());
                for (i, result) in results.iter().enumerate() {
                    println!(
                        "  {}. {} - {} {:?} {:?}",
                        i + 1,
                        result.title,
                        result.url,
                        result.snippet,
                        result.raw,
                    );
                }
            }
            Err(e) => {
                println!("❌ Search failed: {}", e);
                println!("💡 Tip: Check if your proxy is running on 127.0.0.1:7897");
            }
        }
    }

    #[tokio::test]
    async fn test_search_search_tool() {
        // enable_test_log();

        // 打印当前代理设置
        println!("Current proxy settings:");
        println!("  http_proxy:  {:?}", env::var("http_proxy").ok());
        println!("  https_proxy: {:?}", env::var("https_proxy").ok());
        println!("  all_proxy:   {:?}", env::var("all_proxy").ok());

        // 如果没有设置代理，手动设置（仅用于测试）
        if env::var("https_proxy").is_err() {
            println!("⚠️  No proxy detected, setting default proxy...");
            env::set_var("http_proxy", "http://127.0.0.1:7897");
            env::set_var("https_proxy", "http://127.0.0.1:7897");
            env::set_var("all_proxy", "socks5://127.0.0.1:7897");
        }
        let req = &OpenAIReq {
            url: "https://api.openai-proxy.org/v1".into(),
            api_key: "sk-egR0EUYn0EzVybl7D9JkUFx2ksnD62sZPQ8d0J0yJgPEiAWt".into(),
            model: "gpt-4.1-mini".into(),
            msg: r#"
                tell me what time and date is in new york
                "#
            .into(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            images: vec![],
            search_engine: Default::default(),
        };

        let p = Arc::new(|x| println!("{}", x));

        call_openai_stream(
            req,
            vec![Box::new(WebSearchTool {
                engine: Box::new(websearch::providers::DuckDuckGoProvider::new()),
                emit: p.clone(),
            })],
            p,
            |x| x.to_owned(),
        )
        .await
        .unwrap();
    }
}
