use base64::Engine;

use dbnexus_db::db::{BackendKind, ExportFrame, ExportKind, TimeElapsed};
use dbnexus_db::mongo::MongoDB;

use dbnexus_db::redis::RedisDB;
use futures_util::StreamExt;
use mongodb::bson::doc;
use rig::agent::{Agent, MultiTurnStreamItem, PromptHook, StreamingError, Text};
use rig::client::completion::CompletionClient;
use rig::loaders::StripXmlProcessor;
use rig::providers::openai::responses_api::ResponsesCompletionModel;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};
use rig::tool::ToolDyn;
use rig::wasm_compat::{WasmCompatSend, WasmCompatSync};
use rig::EmptyListError;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::app::{
    self, AISteamResp, AiOnlineSearch, DBPal, DatabaseBackend, ErrorFrontend, OpenAIReq,
    TaskManager,
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

fn make_agent(
    req: &app::OpenAIReq,
    tools: Vec<Box<dyn ToolDyn>>,
    preamble: Option<&str>,
) -> anyhow::Result<Agent<ResponsesCompletionModel>> {
    #[cfg(debug_assertions)]
    log::info!("open ai req:{:?}", req);
    let builder = openai::Client::builder()
        .api_key(&req.api_key)
        .base_url(&req.url);
    let api: Client<openai::OpenAIResponsesExt> = builder.build()?;
    let mut agent_builder = api.agent(&req.model);
    if let Some(t) = req.max_tokens {
        agent_builder = agent_builder.max_tokens(t);
    }
    if let Some(t) = preamble {
        agent_builder = agent_builder.preamble(t);
    }
    if let Some(t) = req.temperature {
        agent_builder = agent_builder.temperature(t);
    }
    // TODO how to set this???
    agent_builder = agent_builder.default_max_turns(20);

    let agent = agent_builder.tools(tools).build();
    anyhow::Ok(agent)
}
#[specta::specta]
#[tauri::command]
pub async fn openai_invoke(req: app::OpenAIReq) -> Result<String, ErrorFrontend> {
    let api = make_agent(&req, vec![], None).map_err(ErrorFrontend::from)?;
    let r = api.prompt(&req.prompt).await?;
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
    agent: Agent<ResponsesCompletionModel>,
    emit: Arc<dyn Fn(AISteamResp) + Send + Sync + 'static>,
    comment_fn: impl Fn(&str) -> String + Send + Sync + 'static,
    prompt: Message,
) -> Result<(), ErrorFrontend> {
    let mut prompt = agent.stream_prompt(prompt).await;
    while let Some(chunk) = prompt.next().await {
        let chunk = chunk?;
        match chunk {
            MultiTurnStreamItem::StreamAssistantItem(item) => match item {
                rig::streaming::StreamedAssistantContent::Text(text) => {
                    emit(text.text().to_owned().into());
                }
                StreamedAssistantContent::ToolCall { .. } => {
                    // tool call have notice
                }
                StreamedAssistantContent::ToolCallDelta { .. } => {}
                StreamedAssistantContent::Reasoning(reasoning) => {
                    reasoning.content.iter().for_each(|x| match x {
                        ReasoningContent::Text { text, signature: _ } => {
                            emit(text.clone().into());
                        }
                        ReasoningContent::Encrypted(_) => {}
                        ReasoningContent::Redacted { .. } => {}
                        ReasoningContent::Summary(summary) => emit(summary.clone().into()),
                        _ => {
                            log::error!("maybe handle type:{:?}", x);
                        }
                    })
                }
                StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
                    emit(reasoning.into())
                }
                StreamedAssistantContent::Final(x) => {
                    emit(AISteamResp::TokenUsage {
                        input_token: x.usage.input_tokens,
                        output_token: x.usage.output_tokens,
                        total_token: x.usage.total_tokens,
                    });
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
                            ToolResultContent::Text(text) => emit(
                                comment_fn(if let Some(x) = text.text().get(0..20) {
                                    x
                                } else {
                                    text.text()
                                })
                                .into(),
                            ),
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
    emit: ToolEmitter,
}
impl<M> PromptHook<M> for EmitSomeStatus where M: CompletionModel + 'static {}

impl DBPal {
    async fn make_tools(
        &self,
        name: &str,
        query_id: impl Into<String>,
        emit: ToolEmitter,
    ) -> anyhow::Result<Vec<Box<dyn ToolDyn>>> {
        let db = self.get_backend(name).await?;
        match db.backend_kind() {
            BackendKind::Redis => {
                let x = db
                    .as_any()
                    .downcast_ref::<RedisDB>()
                    .expect("must be redis");
                let client = Clone::clone(x);
                let r: Vec<Box<dyn ToolDyn>> = vec![Box::new(redis::RedisTool {
                    client,
                    query_id: query_id.into(),
                })];
                anyhow::Ok(r)
            }
            BackendKind::Mongo => {
                let x = db
                    .as_any()
                    .downcast_ref::<MongoDB>()
                    .expect("must be mongodb");
                let c = Clone::clone(x);
                let r: Vec<Box<dyn ToolDyn>> = vec![
                    Box::new(mongo_tool::MongoQueryTool {
                        client: c,
                        max_limit: 10_000,
                    }),
                    Box::new(mongo_tool::MongoRunCommandTool {
                        client: Clone::clone(x),
                    }),
                ];
                anyhow::Ok(r)
            }

            _ => {
                // sql base database
                let r: Vec<Box<dyn ToolDyn>> = vec![Box::new(SqlQueryTool {
                    db,
                    query_id: query_id.into(),
                    emit: emit.clone(),
                })];
                anyhow::Ok(r)
            }
        }
    }
}

pub struct SqlQueryTool {
    db: DatabaseBackend,
    query_id: String,
    emit: ToolEmitter,
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

impl From<mongodb::error::Error> for CallToolErr {
    fn from(value: mongodb::error::Error) -> Self {
        Self {
            err_msg: value.to_string(),
        }
    }
}
impl From<String> for CallToolErr {
    fn from(value: String) -> Self {
        Self { err_msg: value }
    }
}
impl From<&str> for CallToolErr {
    fn from(value: &str) -> Self {
        Self {
            err_msg: value.into(),
        }
    }
}

impl From<anyhow::Error> for CallToolErr {
    fn from(value: anyhow::Error) -> Self {
        Self {
            err_msg: value.to_string(),
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

/// tool emitter , impl should comment and somehow send to user
pub type ToolEmitter = Arc<dyn Fn(String) + Send + Sync + 'static>;

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
        (self.emit)(format!("\ncall tool sql query {}", args.query));
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
            (self.emit)("data exceed 100,some data not feed to LLM".to_string())
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

/// Arguments for the web fetch tool
#[derive(Deserialize, Serialize, Debug)]
pub struct WebFetchArgs {
    /// The URL to fetch content from
    pub url: String,
}

/// Output structure containing the fetched content
#[derive(Deserialize, Serialize, Debug)]
pub struct WebFetchOutput {
    /// The URL that was fetched
    pub url: String,
    /// The content type of the response (e.g., text/html, application/json)
    pub content_type: Option<String>,
    /// The raw content fetched from the URL
    pub content: String,
    /// HTTP status code
    pub status: u16,
}

/// A tool that fetches content from a given URL
///
/// This tool can be used to retrieve web pages, API responses, or any HTTP-accessible content.
/// It handles HTTP GET requests and returns the response body along with metadata.
#[derive(Clone)]
pub struct WebFetchTool {
    // Optional: configure timeout, headers, etc.
    client: reqwest::Client,
    emit: ToolEmitter,
}

impl WebFetchTool {
    /// Create a new instance with default configuration
    pub fn new(emit: ToolEmitter) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("rig-core-agent/1.0")
            .build()?;

        anyhow::Ok(Self { client, emit })
    }
}

impl From<reqwest::Error> for CallToolErr {
    fn from(value: reqwest::Error) -> Self {
        CallToolErr {
            err_msg: value.to_string(),
        }
    }
}

impl Tool for WebFetchTool {
    const NAME: &'static str = "fetch_url";

    type Error = CallToolErr;
    type Args = WebFetchArgs;
    type Output = WebFetchOutput;

    /// Returns the tool definition that describes the function to the LLM
    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: concat!(
                "Fetches and retrieves the content of a web page or API endpoint from a given URL. ",
                "Use this tool when you need to access external web content, retrieve data from APIs, ",
                "or read the content of a specific webpage. ",
                "The tool returns the raw HTML, JSON, or text content along with HTTP status information. ",
                "Supports standard HTTP/HTTPS URLs."
            ).to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The complete URL to fetch (e.g., https://api.example.com/data or https://example.com/page). Must include protocol (http:// or https://).",
                        "format": "uri"
                    }
                },
                "required": ["url"]
            }),
        }
    }

    /// Executes the web fetch operation
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if !args.url.starts_with("http://") && !args.url.starts_with("https://") {
            return Err(CallToolErr::from(
                "URL must start with http:// or https://".to_string(),
            ));
        }
        (self.emit)(format!("fetching url: {}\n", args.url));
        //  Execute HTTP GET request
        let response = self
            .client
            .get(&args.url)
            .send()
            .await
            .map_err(|e| CallToolErr::from(e.to_string()))?;

        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Get text content (handles encoding automatically)
        let content = response
            .text()
            .await
            .map_err(|e| CallToolErr::from(e.to_string()))?;
        (self.emit)(format!(
            "fetched url: {} content: {}\n",
            args.url,
            if let Some(content) = content.get(0..100) {
                content
            } else {
                content.as_str()
            }
        ));
        Ok(WebFetchOutput {
            url: args.url,
            content_type,
            content,
            status,
        })
    }
}

pub struct SteamCache {
    ret: String,
    time_elapsed: TimeElapsed,
    emit: Channel<AISteamResp>,
}

impl SteamCache {
    fn new(emit: Channel<AISteamResp>) -> Self {
        Self {
            ret: String::new(),
            time_elapsed: TimeElapsed::new(Duration::from_millis(400)),
            emit,
        }
    }
}

impl SteamCache {
    fn send(&mut self, x: AISteamResp) {
        match x {
            AISteamResp::Content { msg } => {
                self.ret.push_str(msg.as_str());
                if self.time_elapsed.elapsed() {
                    if let Err(err) = self.emit.send(std::mem::take(&mut self.ret).into()) {
                        log::error!("Error sending message: {:?}", err)
                    }
                }
            }
            AISteamResp::TokenUsage { .. } => {
                if let Err(err) = self.emit.send(x) {
                    log::error!("Error sending message: {:?}", err)
                }
            }
        }
    }
}
impl Drop for SteamCache {
    fn drop(&mut self) {
        if let Err(err) = self.emit.send(std::mem::take(&mut self.ret).into()) {
            log::error!("Error sending message: {:?}", err)
        }
    }
}
#[specta::specta]
#[tauri::command]
pub async fn openai_stream(
    req: OpenAIReq,
    name: String,
    query_id: String,
    emit: Channel<AISteamResp>,
    task_id: String,
    task_manager: tauri::State<'_, TaskManager>,
    dbpal: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let (token, _guard) = task_manager.add(task_id);

    let db = dbpal
        .get_backend(&name)
        .await
        .map_err(ErrorFrontend::from)?;
    let db_kind = db.backend_kind();
    let tool_emitter = {
        let emit = emit.clone();
        Arc::new(move |status: String| {
            if let Err(err) = emit.send(db_kind.comment(&status).into()) {
                log::error!("Error sending message: {:?}", err)
            }
        })
    };
    let mut tools = dbpal
        .make_tools(&name, query_id, tool_emitter.clone())
        .await
        .map_err(ErrorFrontend::from)?;
    if let Some(x) = req.search_engine.make_tool(tool_emitter.clone())? {
        tools.push(x);
    }
    tools.push(Box::new(WebFetchTool::new(tool_emitter.clone())?));
    let agent = make_agent(
        &req,
        tools,
        Some(format!("U a are {} expert", db.backend_kind()).as_str()),
    )?;

    let documents = req.images.iter().map(|x| {
        UserContent::Image(Image {
            data: DocumentSourceKind::Url(x.clone()),
            media_type: None,
            detail: None,
            additional_params: None,
        })
    });

    let msg = Message::User {
        content: {
            let mut content = vec![];
            if let Some(x) = req.user_system_prompt.as_ref() {
                content.push(UserContent::text(x.clone()));
            }
            content.push(UserContent::text(format!(
                "chat_history:\n{}\n",
                req.chat_history
                    .iter()
                    .map(|x| format!("{}:{}\n", x.role, x.msg,))
                    .collect::<String>()
            )));
            if db.backend_kind() == BackendKind::Mongo {
                content.push(UserContent::text(make_mongo_prompt()));
                content.push(UserContent::text("when genarate code avoid top level await,I think my runtime env may not support it."));
            }
            content.push(UserContent::text(format!(
                "system local is :{}",
                sys_locale::get_locale().unwrap_or("en-us".to_string())
            )));
            content.push(UserContent::Text(Text {
                text: req.prompt.clone(),
            }));
            content.extend(
                req.read_documents()
                    .await?
                    .into_iter()
                    .map(UserContent::Document),
            );
            content.extend(documents);
            #[cfg(debug_assertions)]
            {
                for c in content.iter() {
                    println!("user content:{:?}", c);
                }
            }
            OneOrMany::many(content.into_iter())?
        },
    };

    let fur = call_openai_stream(
        agent,
        {
            let cache = Mutex::new(SteamCache::new(emit));
            Arc::new(move |s| {
                cache.lock().expect("lock failed").send(s);
            })
        },
        move |x| db_kind.comment(x),
        msg,
    );
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

impl From<websearch::SearchError> for ErrorFrontend {
    fn from(value: websearch::SearchError) -> Self {
        Self {
            reason: value.to_string(),
            detail: format!("{:?}", value),
            cancelled: false,
        }
    }
}

impl AiOnlineSearch {
    fn make_tool(&self, emit: ToolEmitter) -> Result<Option<Box<dyn ToolDyn>>, ErrorFrontend> {
        if self.enable {
            let engine: Box<dyn websearch::SearchProvider> = match self.provider.as_ref() {
                "duckduckgo" => Box::new(websearch::providers::DuckDuckGoProvider::new()),
                "tavily" => {
                    if let Some(x) = self.api_key.as_ref() {
                        Box::new(websearch::providers::TavilyProvider::new(x)?)
                    } else {
                        return Err(ErrorFrontend::from("tavily need api key."));
                    }
                }
                "serpapi" => {
                    if let Some(x) = self.api_key.as_ref() {
                        Box::new(websearch::providers::SerpApiProvider::new(x)?)
                    } else {
                        return Err(ErrorFrontend::from("serpapi need api key."));
                    }
                }
                "google" => {
                    if let Some(x) = self.api_key.as_ref() {
                        Box::new(websearch::providers::GoogleProvider::new(x, "rig-agent")?)
                    } else {
                        return Err(ErrorFrontend::from("serpapi need api key."));
                    }
                }
                _ => {
                    return Err(ErrorFrontend::from(format!(
                        "search enginenot found:{}",
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

impl OpenAIReq {
    async fn read_document(file: &str) -> anyhow::Result<rig::message::Document> {
        let file = file.to_owned();

        tokio::task::spawn_blocking(move || {
            let d = match &file {
                _ if file.to_lowercase().ends_with(".epub") => {
                    let loader =
                        rig::loaders::EpubFileLoader::<_, StripXmlProcessor>::with_glob(&file)
                            .map_err(|err| anyhow::Error::msg(err.to_string()))?;

                    // Load epub file contents by chapter, ignoring any errors
                    let contents = loader
                        .load_with_path()
                        .ignore_errors()
                        .by_chapter()
                        .ignore_errors();
                    let mut doc: String = String::new();
                    for (path, chapters) in contents {
                        doc.push_str(format!("file {:?} begins\n", path).as_str());
                        for (idx, chapter) in chapters {
                            doc.push_str(format!("Chapter {} begins\n", idx).as_str());
                            doc.push_str(format!("{}\n", chapter).as_str());
                            doc.push_str(format!("Chapter {} ends\n", idx).as_str());
                        }
                        doc.push_str(format!("file {:?} ends\n", path).as_str());
                    }
                    rig::message::Document {
                        data: DocumentSourceKind::String(doc),
                        media_type: None,
                        additional_params: None,
                    }
                }
                _ if file.to_lowercase().ends_with(".pdf") => {
                    let loader = rig::loaders::PdfFileLoader::with_glob(&file)
                        .map_err(|err| anyhow::Error::msg(err.to_string()))?;
                    // Load pdf file contents by page, ignoring any errors
                    let contents = loader
                        .load_with_path()
                        .ignore_errors()
                        .by_page()
                        .ignore_errors();
                    let mut doc = String::new();
                    for (path, chapters) in contents {
                        doc.push_str(format!("file {:?} begins\n", path).as_str());
                        for (idx, chapter) in chapters {
                            doc.push_str(format!("Chapter {} begins\n", idx).as_str());
                            doc.push_str(format!("{}\n", chapter).as_str());
                            doc.push_str(format!("Chapter {} ends\n", idx).as_str());
                        }
                        doc.push_str(format!("file {:?} ends\n", path).as_str());
                    }

                    rig::message::Document {
                        data: DocumentSourceKind::String(doc),
                        media_type: None,
                        additional_params: None,
                    }
                }
                _ => {
                    let doc = std::fs::read_to_string(&file)?;
                    rig::message::Document {
                        data: DocumentSourceKind::String(doc),
                        media_type: None,
                        additional_params: None,
                    }
                }
            };
            anyhow::Ok(d)
        })
        .await?
    }
    async fn read_documents(&self) -> anyhow::Result<Vec<rig::message::Document>> {
        let mut documents = vec![];

        for file in self.documents.iter() {
            let file = file.to_lowercase();
            println!("file:{}", file);
            let d = Self::read_document(&file).await?;
            documents.push(d);
        }
        anyhow::Ok(documents)
    }
}

/// 单个搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchInput {
    /// 搜索查询字符串
    pub query: String,
    pub max_results: Option<u32>,
}

/// 搜索工具的输出结果
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    emit: ToolEmitter,
}

impl Tool for WebSearchTool {
    const NAME: &'static str = "web_search";

    type Error = CallToolErr;

    type Args = SearchInput;

    type Output = SearchOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
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

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        (self.emit)(format!("search internet for answer,query:{}\n", args.query));

        let start = std::time::Instant::now();
        let results = self
            .engine
            .search(&websearch::SearchOptions {
                query: args.query.clone(),
                max_results: args.max_results,
                ..Default::default()
            })
            .await
            .map_err(|err| CallToolErr::from(format!("call search engine failed,err:{}", err)))?;

        #[cfg(debug_assertions)]
        {
            for r in results.iter() {
                println!("search result-{}-{:?} ", r.title, r.snippet);
            }
        }

        (self.emit)(format!("search done:{} results {} ...\n", results.len(), {
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
        }));

        Ok(SearchOutput {
            query: args.query.clone(),
            search_time_ms: start.elapsed().as_millis() as u64,
            results,
        })
    }
}

mod mongo_tool {

    use dbnexus_db::mongo::MongoDB;
    use futures_util::StreamExt;
    use mongodb::{
        bson::{doc, Document},
        Cursor,
    };
    use rig::{completion::ToolDefinition, tool::Tool};
    use serde::{Deserialize, Serialize};

    use crate::ai::CallToolErr;

    pub struct MongoQueryTool {
        pub(crate) client: MongoDB,

        // Safety limits
        pub(crate) max_limit: i64,
    }

    /// LLM-generated query request
    #[derive(Debug, Deserialize, Serialize)]
    pub struct MongoQueryRequest {
        /// Collection name
        pub collection: String,

        /// Operation type: find, aggregate, count, distinct, etc.
        pub operation: String,

        /// Query filter (BSON format)
        #[serde(default)]
        pub filter: Option<Document>,

        /// Aggregation pipeline (for aggregate operations)
        #[serde(default)]
        pub pipeline: Option<Vec<Document>>,

        /// Field projection
        #[serde(default)]
        pub projection: Option<Document>,

        /// Sort order
        #[serde(default)]
        pub sort: Option<Document>,

        /// Pagination limit
        #[serde(default)]
        pub limit: Option<i64>,

        /// Records to skip
        #[serde(default)]
        pub skip: Option<u64>,
    }

    impl Tool for MongoQueryTool {
        const NAME: &'static str = "mongo_query";

        type Args = MongoQueryRequest;
        type Output = Vec<Document>;

        async fn call(&self, args: Self::Args) -> Result<Self::Output, CallToolErr> {
            let db = self.client.get_db().await?;

            let MongoQueryRequest {
                collection: collection_name,
                operation,
                filter,
                pipeline,
                projection,
                sort,
                limit,
                skip,
            } = args;

            let collection = db.collection::<Document>(&collection_name);

            // Limit return count to prevent database overload
            let limit = limit.unwrap_or(10).min(self.max_limit);

            let result = match operation.as_str() {
                "find" => {
                    let mut builder = collection.find(filter.unwrap_or_default());

                    if let Some(proj) = projection {
                        builder = builder.projection(proj);
                    }
                    if let Some(sort) = sort {
                        builder = builder.sort(sort);
                    }
                    builder = builder.limit(limit);
                    if let Some(skip) = skip {
                        builder = builder.skip(skip);
                    }
                    let mut cursor = builder.await?;
                    let mut ret = Vec::new();
                    while let Some(doc) = cursor.next().await {
                        let doc = doc?;
                        ret.push(doc);
                    }
                    ret
                }

                "aggregate" => {
                    let pipeline = pipeline.unwrap_or_default();
                    let cursor = collection.aggregate(pipeline);
                    let mut cursor: Cursor<Document> =
                        cursor.await.map_err(|e| CallToolErr::from(e.to_string()))?;
                    let mut docs = vec![];
                    while let Some(x) = cursor.next().await {
                        let x = x.map_err(|e| CallToolErr::from(e.to_string()))?;
                        docs.push(x);
                    }
                    docs
                }

                "count" => {
                    let count = collection
                        .count_documents(filter.unwrap_or_default())
                        .await?;
                    vec![doc! { "count": count as i64 }]
                }
                "distinct" => {
                    let field_name = projection
                        .and_then(|p| p.keys().next().cloned())
                        .ok_or_else(|| {
                            CallToolErr::from("distinct requires field name in projection")
                        })?;
                    let result = db
                        .run_command(doc! {
                            "distinct": &collection_name,
                            "key": field_name,
                            "query": filter.unwrap_or_default()
                        })
                        .await
                        .map_err(|e| CallToolErr::from(e.to_string()))?;

                    vec![result] // 包装成数组保持返回类型一致
                }

                _ => return Err(format!("Unsupported operation: {}", operation).into()),
            };

            Ok(result) as Result<Self::Output, Self::Error>
        }

        async fn definition(&self, _: String) -> ToolDefinition {
            ToolDefinition {
                name: Self::NAME.to_string(),
                description: "Execute MongoDB queries. Supports find/aggregate/count operations. \
                Example: {\"collection\":\"users\",\"operation\":\"find\",\
                \"filter\":{\"age\":{\"$gt\":18}},\"limit\":10}"
                    .to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "collection": { "type": "string", "description": "Collection name" },
                        "operation": {
                            "type": "string",
                            "enum": ["find", "aggregate", "count", "distinct"],
                            "description": "Query operation type"
                        },
                        "filter": {
                            "type": "object",
                            "description": "Query filter in MongoDB syntax"
                        },
                        "pipeline": {
                            "type": "array",
                            "description": "Aggregation pipeline (for aggregate operation)"
                        },
                        "projection": {
                            "type": "object",
                            "description": "Field projection, e.g. {\"name\": 1, \"_id\": 0}"
                        },
                        "sort": {
                            "type": "object",
                            "description": "Sort order, e.g. {\"created_at\": -1}"
                        },
                        "limit": { "type": "integer", "description": "Maximum records to return" },
                        "skip": { "type": "integer", "description": "Records to skip" }
                    },
                    "required": ["collection", "operation"]
                }),
            }
        }
        type Error = CallToolErr;
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct RunCommandArgs {
        /// MongoDB command document to execute. First key must be the command name.
        /// Examples: {"serverStatus": 1}, {"createIndex": "coll", "key": {"f": 1}}
        pub command: Document,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct RunCommandOutput {
        /// Raw response from MongoDB
        pub result: Document,
    }
    #[derive(Clone)]
    pub struct MongoRunCommandTool {
        pub(crate) client: MongoDB,
    }

    impl Tool for MongoRunCommandTool {
        const NAME: &'static str = "mongodb_run_command";

        type Args = RunCommandArgs;
        type Output = RunCommandOutput;
        type Error = CallToolErr;

        async fn definition(&self, _prompt: String) -> ToolDefinition {
            ToolDefinition {
                name: "mongodb_run_command".to_string(),
                description:
                    "Execute a raw MongoDB database command and return the server response."
                        .to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "object",
                            "description": "MongoDB command document to execute. First key must be the command name. Examples: {\"serverStatus\": 1}, {\"dbStats\": 1}, {\"createIndexes\": \"users\", \"indexes\": [{\"key\": {\"email\": 1}, \"unique\": true}]}, {\"aggregate\": \"orders\", \"pipeline\": [{\"$group\": {\"_id\": \"$status\"}}], \"cursor\": {}}"
                        }
                    },
                    "required": ["command"]
                }),
            }
        }

        async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
            let result = self
                .client
                .get_db()
                .await?
                .run_command(args.command)
                .await?;
            Ok(RunCommandOutput { result })
        }
    }
}

mod redis {
    use std::ops::DerefMut;

    use dbnexus_db::redis::{command_return_multi_value, RedisDB};
    use futures_util::StreamExt;
    use rig::tool::Tool;
    use serde::{Deserialize, Deserializer, Serialize};
    use serde_json::json;

    use crate::ai::CallToolErr;

    #[derive(Deserialize, Serialize, Debug)]
    pub struct RedisCommandArgs {
        pub command: String,
        #[serde(deserialize_with = "deserialize_maybe_string_array")]
        pub args: Option<Vec<String>>,
    }
    /// 自定义反序列化：兼容 ["a"] 和 "[\"a\"]"
    fn deserialize_maybe_string_array<'de, D>(
        deserializer: D,
    ) -> Result<Option<Vec<String>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde_json::Value;

        // 先解析为通用的 JSON Value
        let v: Value = Deserialize::deserialize(deserializer)?;

        match v {
            // 情况 1: 已经是标准的数组 [ "arg1" ]
            Value::Array(arr) => {
                let res = arr
                    .into_iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect();
                Ok(Some(res))
            }
            // 情况 2: 模型传了一个字符串 "[\"arg1\"]"
            Value::String(s) => {
                let parsed: Vec<String> =
                    serde_json::from_str(&s).map_err(serde::de::Error::custom)?;
                Ok(Some(parsed))
            }
            // 其他情况（如 Null）返回 None
            _ => Ok(None),
        }
    }
    // 2. 定义你的 Tool 结构体
    pub struct RedisTool {
        pub(crate) query_id: String,
        pub(crate) client: RedisDB,
    }

    // 3. 实现 Tool Trait
    impl Tool for RedisTool {
        const NAME: &'static str = "redis_executor";

        type Error = CallToolErr;
        type Args = RedisCommandArgs;
        type Output = Vec<String>;

        async fn definition(&self, _: String) -> rig::completion::ToolDefinition {
            rig::completion::ToolDefinition {
                name: Self::NAME.to_string(),
                description: "执行 Redis 命令。例如: command='GET', args=['key']".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Redis 命令名称 (如: GET, SET, INFO, FLUSHDB)"
                        },
                        "args": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "命令参数列表。注意：必须是真实的 JSON 数组格式，例如 [\"key1\", \"value1\"]，不要传字符串形式的数组 ,如果参数
                                            是int或者float请转化成string
                                            "
                        }
                    },
                    // 关键改动：这里只保留 command 为必填
                    "required": ["command"]
                }),
            }
        }

        async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
            let RedisCommandArgs { command, args } = args;
            let conn = self.client.get_query_conn_by_id(&self.query_id)?;
            let mut conn = conn.lock().await;
            let mut redis_cmd = redis::cmd(&command);
            redis_cmd.arg(args);
            let mut ret = vec![];
            if command_return_multi_value(&command) {
                let mut stream = redis_cmd.iter_async::<String>(conn.deref_mut()).await?;
                while let Some(x) = stream.next().await {
                    ret.push(x?);
                }
            } else {
                let value = redis_cmd.query_async::<String>(conn.deref_mut()).await?;
                ret.push(value);
            }
            Ok(ret)
        }
    }

    impl From<redis::RedisError> for CallToolErr {
        fn from(value: redis::RedisError) -> Self {
            Self {
                err_msg: value.to_string(),
            }
        }
    }
}

fn make_mongo_prompt() -> String {
    format!(
        "we have some predefine some variable u can use.\n\
        ```typescript\n\
        {}\n\
        {}\n\
        ```",
        include_str!("../../src/comp/MongoQueryBackend.ts"),
        include_str!("../../src/comp/MongoVar.ts"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbnexus_db::{
        datasource_config::SqliteDBConfig,
        db::{QuerySql, SchemaTrait},
        sqlite::{sqlite_test_file_path, SqliteDB},
    };
    use futures_util::StreamExt;
    use rig::{
        agent::MultiTurnStreamItem,
        streaming::{StreamedAssistantContent, StreamedUserContent, StreamingChat},
    };
    use websearch::SearchProvider;

    #[test]

    fn test_mongo_prompt() {
        println!("{}", make_mongo_prompt());
    }
    #[tokio::test]

    async fn test_xxx() {
        let req = &OpenAIReq {
            url: "https://dashscope.aliyuncs.com/compatible-mode/v1".into(),
            api_key: "sk-6c19c965d3234c8fae49c19e4f0301f0".into(),
            model: "qwen3.5-plus".into(),
            prompt: "hello can u help me".into(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            images: vec![],
            search_engine: Default::default(),
            chat_history: vec![],
            user_system_prompt: None,
            documents: vec![],
        };
        let agent = make_agent(req, vec![], None).unwrap();
        call_openai_stream(
            agent,
            Arc::new(|x| print!("{}", x)),
            |x| x.to_owned(),
            Message::user("can u help me."),
        )
        .await
        .unwrap();
    }

    #[tokio::test]

    async fn test_openai_prompt() {
        let now = std::time::Instant::now();

        let req = &OpenAIReq {
            url: "https://api.openai-proxy.org/v1".into(),
            api_key: "sk-egR0EUYn0EzVybl7D9JkUFx2ksnD62sZPQ8d0J0yJgPEiAWt".into(),
            model: "gpt-5".into(),
            prompt: "hello can u help me".into(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            images: vec![],
            search_engine: Default::default(),
            chat_history: vec![],
            documents: vec![],
            user_system_prompt: None,
        };
        let agent: Agent<ResponsesCompletionModel> = make_agent(req, vec![], None).unwrap();
        println!("{:?}", now.elapsed());
        let r = agent.prompt("hello").await.unwrap();
        println!("{:?}", now.elapsed());
        dbg!(r);
    }

    #[tokio::test]

    async fn test_fine_with_tool() {
        let db = SqliteDB::new(SqliteDBConfig {
            path: sqlite_test_file_path(),
            read_only: true,
            conn_timeout: None,
        })
        .await
        .unwrap();
        let req = &OpenAIReq {
            user_system_prompt: None,
            url: "https://dashscope.aliyuncs.com/compatible-mode/v1".into(),
            api_key: "sk-6c19c965d3234c8fae49c19e4f0301f0".into(),
            model: "qwen3.5-plus".into(),
            chat_history: vec![],
            prompt: r#"
human: tell me Mike's age from TABLE Persons 
                "#
            .into(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            images: vec![],
            search_engine: Default::default(),
            documents: vec![],
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

        let r = agent.prompt(&req.prompt).await.unwrap();

        println!("{}", r);
    }

    #[tokio::test]

    async fn test_steam_not_ok() {
        let db = SqliteDB::new(SqliteDBConfig {
            path: sqlite_test_file_path(),
            read_only: true,
            conn_timeout: None,
        })
        .await
        .unwrap();

        let req = &OpenAIReq {
            chat_history: vec![],
            url: "https://api.openai-proxy.org/v1".into(),
            api_key: "sk-egR0EUYn0EzVybl7D9JkUFx2ksnD62sZPQ8d0J0yJgPEiAWt".into(),
            model: "gpt-4.1-mini".into(),
            prompt: r#"
human: tell me Mike's age from TABLE Persons 
                "#
            .into(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            images: vec![],
            search_engine: Default::default(),
            user_system_prompt: None,
            documents: vec![],
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
        let mut stream = agent
            .stream_chat(req.prompt.clone(), vec![] as Vec<String>)
            .await;

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
            chat_history: vec![],
            url: "https://dashscope.aliyuncs.com/compatible-mode/v1".into(),
            api_key: "sk-6c19c965d3234c8fae49c19e4f0301f0".into(),
            model: "qwen3.6-plus".into(),
            prompt: r#"
                tell me what time and date is in new york
                "#
            .into(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            images: vec![],
            search_engine: Default::default(),
            user_system_prompt: None,
            documents: vec![],
        };

        let print_steam = Arc::new(|x: AISteamResp| print!("{}", x));

        let print_emit = Arc::new(|x: String| println!("{}", x));

        {
            println!("testing duckduckgo search tool...");
            let agent = make_agent(
                req,
                vec![Box::new(WebSearchTool {
                    engine: Box::new(websearch::providers::DuckDuckGoProvider::new()),
                    emit: print_emit.clone(),
                })],
                None,
            )
            .unwrap();

            let msg = Message::user(&req.prompt);

            call_openai_stream(agent, print_steam.clone(), |x| x.to_owned(), msg)
                .await
                .unwrap();
        }

        //

        {
            println!("testing search tool...");
            let agent = make_agent(
                req,
                vec![Box::new(WebSearchTool {
                    engine: Box::new(
                        websearch::providers::TavilyProvider::new(
                            "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",
                        )
                        .unwrap(),
                    ),
                    emit: print_emit.clone(),
                })],
                None,
            )
            .unwrap();

            let msg = Message::user(&req.prompt);

            call_openai_stream(agent, print_steam.clone(), |x| x.to_owned(), msg)
                .await
                .unwrap();
        }

        {
            println!("testing tavily serpapi tool...");
            let agent = make_agent(
                req,
                vec![Box::new(WebSearchTool {
                    engine: Box::new(
                        websearch::providers::SerpApiProvider::new(
                            "470cd3e4754aa5b76b04e80ddc85520e1d953cd303d6487ebe7c949e6629fbc4",
                        )
                        .unwrap(),
                    ),
                    emit: print_emit.clone(),
                })],
                None,
            )
            .unwrap();

            let msg = Message::user(&req.prompt);

            call_openai_stream(agent, print_steam.clone(), |x| x.to_owned(), msg)
                .await
                .unwrap();
        }
        {
            println!("testing google tool...");
            let agent = make_agent(
                req,
                vec![Box::new(WebSearchTool {
                    engine: Box::new(
                        websearch::providers::GoogleProvider::new(
                            "470cd3e4754aa5b76b04e80ddc85520e1d953cd303d6487ebe7c949e6629fbc4",
                            "rig-agent",
                        )
                        .unwrap(),
                    ),
                    emit: print_emit.clone(),
                })],
                None,
            )
            .unwrap();

            let msg = Message::user(&req.prompt);

            call_openai_stream(agent, print_steam.clone(), |x| x.to_owned(), msg)
                .await
                .unwrap();
        }
    }

    #[tokio::test]

    async fn test_mongo_tool() {
        let db = dbnexus_db::mongo::make_test_object();
        let col = "Person";
        let _ = db.drop_table(col, false).await;
        db.query_collection_insert_many(
            col,
            vec![
                doc! {
                    "name": "Alice",
                    "age" : 18,
                },
                doc! {
                    "name": "Bob",
                    "age" : 19,
                },
                doc! {
                    "name": "Mike",
                    "age" : 55,
                },
            ],
        )
        .await
        .unwrap();

        let req = &OpenAIReq {
            user_system_prompt: None,
            url: "https://dashscope.aliyuncs.com/compatible-mode/v1".into(),
            api_key: "sk-6c19c965d3234c8fae49c19e4f0301f0".into(),
            model: "qwen3.5-plus".into(),
            chat_history: vec![],
            prompt: r#""#.into(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            images: vec![],
            search_engine: Default::default(),
            documents: vec![],
        };

        let emit = Arc::new(|x| print!("{}", x));

        {
            //
            let agent = make_agent(
                req,
                vec![
                    Box::new(mongo_tool::MongoQueryTool {
                        client: Clone::clone(&db),
                        max_limit: 10,
                    }),
                    Box::new(mongo_tool::MongoRunCommandTool {
                        client: Clone::clone(&db),
                    }),
                ],
                Some("u are a mongo export"),
            )
            .unwrap();

            //

            println!("=============================== tell me the server version.");
            call_openai_stream(
                agent.clone(),
                emit.clone(),
                |x| x.to_string(),
                Message::user("tell me the server version."),
            )
            .await
            .unwrap();

            println!("=========================================================== ");
            call_openai_stream(
                agent.clone(),
                emit.clone(),
                |x| x.to_string(),
                Message::user("tell me who is the oldest in table Person"),
            )
            .await
            .unwrap();
        }
    }

    #[tokio::test]
    async fn test_read_documents() {
        // 1. 修正路径拼接：使用 PathBuf 更稳健，去掉多余引号
        let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("..");
        path.push("tests");
        let test_path = path.clone();
        {
            let mut test_path = test_path.clone();
            test_path.push("minimal-v2.epub");
            let d = OpenAIReq::read_document(test_path.to_str().unwrap())
                .await
                .unwrap();
            match &d.data {
                DocumentSourceKind::String(origin) => {
                    println!("origin:{}", origin);
                    let check = origin.to_lowercase();
                    assert!(check.contains("section 1"));
                    assert!(check.contains("this is "));
                }
                _ => {
                    unreachable!()
                }
            }
        }

        {
            let mut test_path = test_path.clone();
            test_path.push("dummy.pdf");
            let d = OpenAIReq::read_document(test_path.to_str().unwrap())
                .await
                .unwrap();
            match &d.data {
                DocumentSourceKind::String(origin) => {
                    println!("origin:{}", origin);
                    let check = origin.to_lowercase();
                    assert!(check.contains("dummy"));
                }
                _ => {
                    unreachable!()
                }
            }
        }
    }
}
