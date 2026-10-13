use base64::Engine;
use dbnexus_db::couchbase::CouchBaseDB;
use dbnexus_db::db::{BackendKind, ExportFrame, ExportKind, TimeElapsed};
use dbnexus_db::mongo::MongoDB;
use dbnexus_db::redis::RedisDB;
use futures_util::StreamExt;
use mongodb::bson::doc;
use rig::agent::{Agent, MultiTurnStreamItem, Text};
use rig::loaders::StripXmlProcessor;
use rig::AgentBuilder;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::app::{
    self, AISteamResp, AiOnlineSearch, DBPal, DatabaseBackend, ErrorFrontend, OpenAIReq,
    TaskManager,
};
use rig::completion::{Message, PromptError};
use rig::message::{DocumentSourceKind, Image, UserContent};
use rig::providers::openai;
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

type AgentBuilderWithTools = AgentBuilder<rig::agent::WithBuilderTools>;

fn make_agent_builder(
    req: &app::OpenAIReq,
    preamble: Option<&str>,
) -> anyhow::Result<AgentBuilderWithTools> {
    #[cfg(debug_assertions)]
    log::info!("open ai req:{:?}", req);
    let openai_client = openai::OpenAIConfig::new(req.api_key.clone())
        .with_base_url(&req.url)
        .client();

    let mut agent_builder = AgentBuilder::new(openai_client.completion(&req.model));

    if let Some(t) = req.max_tokens {
        agent_builder = agent_builder.max_tokens(t as u64);
    }
    if let Some(t) = preamble {
        agent_builder = agent_builder.preamble(t);
    }
    if let Some(t) = req.temperature {
        agent_builder = agent_builder.temperature(t);
    }
    // TODO how to set this paramater???
    agent_builder = agent_builder.default_max_turns(20);

    let agent_builder = agent_builder.dynamic_tools(vec![]);
    anyhow::Ok(agent_builder)
}

#[specta::specta]
#[tauri::command]
pub async fn openai_invoke(req: app::OpenAIReq) -> Result<String, ErrorFrontend> {
    let api = make_agent_builder(&req, None).map_err(ErrorFrontend::from)?;
    let api = api.build();
    let r = api.prompt(&req.prompt).await?;
    Ok(r.to_string())
}

async fn call_agent_stream(
    agent: &Agent,
    emit: Arc<dyn Fn(AISteamResp) + Send + Sync + 'static>,
    prompt: Message,
) -> Result<(), ErrorFrontend> {
    let mut stream = agent.prompt(prompt).stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        match chunk {
            MultiTurnStreamItem::StreamAssistantItem(item) => match item {
                rig::streaming::Item::Event(e) => match &e {
                    rig::streaming::StreamEvent::Start { .. } => {}
                    rig::streaming::StreamEvent::Text { text, .. } => {
                        emit(text.clone().into());
                    }
                    rig::streaming::StreamEvent::Reasoning { text, .. } => {
                        emit(text.clone().into());
                    }

                    rig::streaming::StreamEvent::Arguments { .. } => {}
                    rig::streaming::StreamEvent::End { .. } => {}
                },
                rig::streaming::Item::Unknown(unknown_payload) => {
                    log::error!("rig returns me unknown payload :{:?}", unknown_payload)
                }
            },
            MultiTurnStreamItem::ToolCall { .. } => {}
            MultiTurnStreamItem::FinalResponse(resp) => {
                let usage = resp.usage();
                emit(AISteamResp::TokenUsage {
                    input_token: usage.input_tokens.map(|x| x as u32),
                    output_token: usage.output_tokens.map(|x| x as u32),
                    total_token: usage.total_tokens.map(|x| x as u32),
                });
            }
            MultiTurnStreamItem::CompletionCall(..) => {}
            _ => {
                log::error!("unhandled stream item: {:?}", chunk)
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

// impl<M> PromptHook<M> for EmitSomeStatus where M: CompletionModel + 'static {}

impl DBPal {
    async fn insert_tools(
        &self,
        name: &str,
        query_id: impl Into<String>,
        emit: ToolEmitter,
        agent_builder: AgentBuilderWithTools,
    ) -> anyhow::Result<AgentBuilderWithTools> {
        let query_id = query_id.into();

        let db = self.get_backend(name).await?;
        match db.backend_kind() {
            BackendKind::Redis => {
                let x = db
                    .as_any()
                    .downcast_ref::<RedisDB>()
                    .expect("must be redis");
                let client = Clone::clone(x);

                let builder = agent_builder.tool(redis::RedisTool { client, query_id });
                anyhow::Ok(builder)
            }

            BackendKind::CouchBase => {
                let x = db
                    .as_any()
                    .downcast_ref::<CouchBaseDB>()
                    .expect("must be couchbase");
                let db = Clone::clone(x);
                let builder = agent_builder.tool(CouchQueryTool {
                    db,
                    emit: emit.clone(),
                });
                anyhow::Ok(builder)
            }

            BackendKind::Mongo => {
                let db = db
                    .as_any()
                    .downcast_ref::<MongoDB>()
                    .expect("must be mongodb");
                let mut builder = agent_builder.tool(mongo_tool::MongoQueryTool {
                    client: Clone::clone(db),
                    max_limit: 10_000,
                });
                builder = builder.tool(mongo_tool::MongoRunCommandTool {
                    client: Clone::clone(db),
                });
                anyhow::Ok(builder)
            }

            _ => {
                let builder = agent_builder.tool(SqlQueryTool {
                    db: Clone::clone(&db),
                    query_id: query_id.clone(),
                    emit: emit.clone(),
                });

                // builder = builder.tool(SqlExecuteTool {
                //     db,
                //     query_id: query_id.clone(),
                //     emit: emit.clone(),
                // });
                anyhow::Ok(builder)
            }
        }
    }
}

pub struct CouchQueryTool {
    emit: ToolEmitter,
    db: CouchBaseDB,
}

impl Tool for CouchQueryTool {
    const NAME: &'static str = "couchbase_query";

    type Args = SqlQueryArgs;
    type Output = SqlQueryOutput;
    type Error = CallToolErr;

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        (self.emit)(format!("\ncall tool couchbase query {}", args.query));
        let (ret, exceed) = self
            .db
            .ai_query_sql(&args.query, 100)
            .await
            .map_err(|e| CallToolErr::from(format!("query failed: {}", e)))?;
        if exceed {
            (self.emit)("data exceed 100,some data not feed to LLM".to_string())
        }
        Ok(SqlQueryOutput {
            result: serde_json::json!({"data": ret }),
            query: args.query,
        })
    }

    fn description(&self) -> String {
        "Executes SQL queries and returns results. Only SELECT read-only queries are supported."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The SQL SELECT query statement to execute"
                }
            },
            "required": ["query"]
        })
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

use rig::tool::{Tool, ToolContext};

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

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
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
            .map_err(|e| CallToolErr::from(format!("query failed: {}", e)))?;
        if exceed {
            (self.emit)("data exceed 100,some data not feed to LLM".to_string())
        }
        let r = expect_export_frame_all_json(ret)?;
        Ok(SqlQueryOutput {
            result: serde_json::json!({"data": r}),
            query: args.query,
        })
    }

    fn description(&self) -> String {
        "执行 SQL 查询并返回结果。只支持 SELECT 只读查询。".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The SQL SELECT query statement to execute"
                }
            },
            "required": ["query"]
        })
    }
}

#[allow(unused)]
pub struct SqlExecuteTool {
    db: DatabaseBackend,
    query_id: String,
    emit: ToolEmitter,
}

impl Tool for SqlExecuteTool {
    const NAME: &'static str = "sql_execute";

    type Args = SqlQueryArgs;
    type Output = SqlQueryOutput;
    type Error = CallToolErr;

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        (self.emit)(format!("\ncall tool sql execute {}", args.query));
        let _ret = self
            .db
            .as_query_sql()
            .execute_sql(&self.query_id, &args.query)
            .await
            .map_err(|e| CallToolErr::from(format!("query failed: {}", e)))?
            .map_err(CallToolErr::from)?;

        Ok(SqlQueryOutput {
            result: serde_json::json!({"data": "execute successfully" }),
            query: args.query,
        })
    }

    fn description(&self) -> String {
        "execute a sql on the database".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The SQL statement to execute"
                }
            },
            "required": ["query"]
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

    /// Executes the web fetch operation
    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
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

    fn description(&self) -> String {
        concat!(
                "Fetches and retrieves the content of a web page or API endpoint from a given URL. ",
                "Use this tool when you need to access external web content, retrieve data from APIs, ",
                "or read the content of a specific webpage. ",
                "The tool returns the raw HTML, JSON, or text content along with HTTP status information. ",
                "Supports standard HTTP/HTTPS URLs."
            ).to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The complete URL to fetch (e.g., https://api.example.com/data or https://example.com/page). Must include protocol (http:// or https://).",
                    "format": "uri"
                }
            },
            "required": ["url"]
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

    let mut agent_builder = make_agent_builder(
        &req,
        Some(format!("U a are {} expert", db.backend_kind()).as_str()),
    )?;

    agent_builder = dbpal
        .insert_tools(&name, query_id, tool_emitter.clone(), agent_builder)
        .await
        .map_err(ErrorFrontend::from)?;
    agent_builder = req
        .search_engine
        .insert_tool(tool_emitter.clone(), agent_builder)?;

    let agent = agent_builder.tool(WebFetchTool::new(tool_emitter.clone())?);

    let documents = req.images.iter().map(|x| {
        UserContent::Image(Image {
            data: DocumentSourceKind::Url(x.clone()),
            media_type: None,
            detail: None,
            native: None,
        })
    });
    let agent = agent.build();

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
            content.push(UserContent::Text(Text::new(req.prompt.clone())));
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
            content
        },
    };

    let fur = call_agent_stream(
        &agent,
        {
            let cache = Mutex::new(SteamCache::new(emit));
            Arc::new(move |s| {
                cache.lock().expect("lock failed").send(s);
            })
        },
        msg,
    );
    tokio::select! {
        res = fur => {res},
        _ = token.cancelled() => {
            Ok(())
        }
    }
}

// Assume path is the local path of an image uploaded by the user.

#[allow(unused)]
async fn image_to_base64(path: &str) -> anyhow::Result<String> {
    let image_data = tokio::fs::read(path).await?;
    let base64_str = base64::engine::general_purpose::STANDARD.encode(image_data);

    let mime = if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else {
        "image/png" // Default
    };
    // The prefix is required to tell the model the image type (for example, image/jpeg or image/png).
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
    fn insert_tool(
        &self,
        emit: ToolEmitter,
        agent_build: AgentBuilderWithTools,
    ) -> Result<AgentBuilderWithTools, ErrorFrontend> {
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
            let builder = agent_build.tool(WebSearchTool { engine, emit });
            Ok(builder)
        } else {
            Ok(agent_build)
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

/// A single search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchInput {
    /// Search query string.
    pub query: String,
    pub max_results: Option<u32>,
}

/// Search tool output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOutput {
    /// Search query.
    pub query: String,
    /// List of search results.
    pub results: Vec<websearch::SearchResult>,
    /// Search duration (milliseconds).
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

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
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

    fn description(&self) -> String {
        "Search the web for information and use the results to answer the user's question. \
                    Use this tool when you need to find current information, facts, news, or specific resources \
                    that may not be in your training data. The tool returns relevant web pages with titles, \
                    URLs, and snippets."
                    .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
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

mod mongo_tool {

    use dbnexus_db::mongo::MongoDB;
    use futures_util::StreamExt;
    use mongodb::{
        bson::{doc, Document},
        Cursor,
    };
    use rig::tool::{Tool, ToolContext};
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

        async fn call(
            &self,
            _context: &mut ToolContext,
            args: Self::Args,
        ) -> Result<Self::Output, CallToolErr> {
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

                    vec![result] // Wrap in an array to keep the return type consistent.
                }

                _ => return Err(format!("Unsupported operation: {}", operation).into()),
            };

            Ok(result) as Result<Self::Output, Self::Error>
        }

        type Error = CallToolErr;

        fn description(&self) -> String {
            "Execute MongoDB queries. Supports find/aggregate/count operations. \
                Example: {\"collection\":\"users\",\"operation\":\"find\",\
                \"filter\":{\"age\":{\"$gt\":18}},\"limit\":10}"
                .to_string()
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({
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
            })
        }
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

        async fn call(
            &self,
            _context: &mut ToolContext,
            args: Self::Args,
        ) -> Result<Self::Output, Self::Error> {
            let result = self
                .client
                .get_db()
                .await?
                .run_command(args.command)
                .await?;
            Ok(RunCommandOutput { result })
        }

        fn description(&self) -> String {
            "Execute a raw MongoDB database command and return the server response.".to_string()
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "object",
                        "description": "MongoDB command document to execute. First key must be the command name. Examples: {\"serverStatus\": 1}, {\"dbStats\": 1}, {\"createIndexes\": \"users\", \"indexes\": [{\"key\": {\"email\": 1}, \"unique\": true}]}, {\"aggregate\": \"orders\", \"pipeline\": [{\"$group\": {\"_id\": \"$status\"}}], \"cursor\": {}}"
                    }
                },
                "required": ["command"]
            })
        }
    }
}

mod redis {
    use std::ops::DerefMut;

    use dbnexus_db::redis::{command_return_multi_value, RedisDB};
    use futures_util::StreamExt;
    use rig::tool::{Tool, ToolContext};
    use serde::{Deserialize, Deserializer, Serialize};
    use serde_json::json;

    use crate::ai::CallToolErr;

    #[derive(Deserialize, Serialize, Debug)]
    pub struct RedisCommandArgs {
        pub command: String,
        #[serde(deserialize_with = "deserialize_maybe_string_array")]
        pub args: Option<Vec<String>>,
    }
    /// Custom deserialization: supports both ["a"] and "[\"a\"]".
    fn deserialize_maybe_string_array<'de, D>(
        deserializer: D,
    ) -> Result<Option<Vec<String>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde_json::Value;

        // First, parse it as a generic JSON Value.
        let v: Value = Deserialize::deserialize(deserializer)?;

        match v {
            // Case 1: This is already a standard array, [ "arg1" ].
            Value::Array(arr) => {
                let res = arr
                    .into_iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect();
                Ok(Some(res))
            }
            // Case 2: The model passed a string, "[\"arg1\"]".
            Value::String(s) => {
                let parsed: Vec<String> =
                    serde_json::from_str(&s).map_err(serde::de::Error::custom)?;
                Ok(Some(parsed))
            }
            // Return None for other cases (such as Null).
            _ => Ok(None),
        }
    }
    // 2. Define the Tool struct.
    pub struct RedisTool {
        pub(crate) query_id: String,
        pub(crate) client: RedisDB,
    }

    // 3. Implement the Tool trait.
    impl Tool for RedisTool {
        const NAME: &'static str = "redis_executor";

        type Error = CallToolErr;
        type Args = RedisCommandArgs;
        type Output = Vec<String>;

        async fn call(
            &self,
            _context: &mut ToolContext,
            args: Self::Args,
        ) -> Result<Self::Output, Self::Error> {
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

        fn description(&self) -> String {
            "执行 Redis 命令。例如: command='GET', args=['key']".to_string()
        }

        fn parameters(&self) -> serde_json::Value {
            json!({
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
                // Key change: only command remains required here.
                "required": ["command"]
            })
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
    use std::env;

    use super::*;
    use dbnexus_db::db::SchemaTrait;

    use websearch::SearchProvider;

    #[test]

    fn test_mongo_prompt() {
        println!("{}", make_mongo_prompt());
    }

    #[derive(Clone, Debug)]

    pub struct AITestUrlModel {
        url: String,
        key: String,
        model: String,
    }

    impl AITestUrlModel {
        fn load_from_dotenv() -> Self {
            dotenvy::dotenv().unwrap();
            Self {
                url: std::env::var("OPENAI_URL").unwrap(),
                key: std::env::var("OPENAI_KEY").unwrap(),
                model: std::env::var("OPENAI_MODEL").unwrap(),
            }
        }
    }

    #[ignore = "reason"]
    #[test]
    fn load_open_ui_url() {
        let x = AITestUrlModel::load_from_dotenv();
        println!("!!:{:?}", x);
    }

    #[tokio::test]

    async fn test_xxx() {
        let config = AITestUrlModel::load_from_dotenv();
        let req = &OpenAIReq {
            url: config.url.clone(),
            api_key: config.key.clone(),
            model: config.model.clone(),
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
        let agent = make_agent_builder(req, None).unwrap();
        let agent = agent.build();

        call_agent_stream(
            &agent,
            Arc::new(|x| println!("{:?}", x)),
            Message::user("can u help me."),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_search_with_proxy() {
        // enable_test_log();

        // Print the current proxy settings.
        println!("Current proxy settings:");
        println!("  http_proxy:  {:?}", env::var("http_proxy").ok());
        println!("  https_proxy: {:?}", env::var("https_proxy").ok());
        println!("  all_proxy:   {:?}", env::var("all_proxy").ok());

        // If no proxy is configured, set one manually (for testing only).
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
        // Print the current proxy settings.
        println!("Current proxy settings:");
        println!("  http_proxy:  {:?}", env::var("http_proxy").ok());
        println!("  https_proxy: {:?}", env::var("https_proxy").ok());
        println!("  all_proxy:   {:?}", env::var("all_proxy").ok());

        // If no proxy is configured, set one manually (for testing only).
        if env::var("https_proxy").is_err() {
            println!("⚠️  No proxy detected, setting default proxy...");
            env::set_var("http_proxy", "http://127.0.0.1:7890");
            env::set_var("https_proxy", "http://127.0.0.1:7890");
            env::set_var("all_proxy", "socks5://127.0.0.1:7890");
        }
        let config = AITestUrlModel::load_from_dotenv();

        let req = &OpenAIReq {
            chat_history: vec![],
            url: config.url.clone(),
            api_key: config.key.clone(),
            model: config.model.clone(),
            prompt: r#"
                who is 周星驰
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

        let print_steam = Arc::new(|x: AISteamResp| print!("{:?}", x));

        let print_emit = Arc::new(|x: String| println!("{}", x));

        {
            println!("testing duckduckgo search tool...");
            let agent = make_agent_builder(req, None).unwrap();

            let agent = agent
                .tool(WebSearchTool {
                    engine: Box::new(websearch::providers::DuckDuckGoProvider::new()),
                    emit: print_emit.clone(),
                })
                .build();

            let msg = Message::user(&req.prompt);

            call_agent_stream(&agent, print_steam.clone(), msg)
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
        let config = AITestUrlModel::load_from_dotenv();

        let req = &OpenAIReq {
            user_system_prompt: None,
            url: config.url.clone(),
            api_key: config.key.clone(),
            model: config.model.clone(),
            chat_history: vec![],
            prompt: r#""#.into(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            images: vec![],
            search_engine: Default::default(),
            documents: vec![],
        };
        let emit = Arc::new(|x| print!("{:?}", x));

        {
            //
            let agent = make_agent_builder(req, Some("u are a mongo export")).unwrap();

            let agent = agent.tool(mongo_tool::MongoQueryTool {
                client: Clone::clone(&db),
                max_limit: 10,
            });

            let agent = agent.tool(mongo_tool::MongoRunCommandTool {
                client: Clone::clone(&db),
            });

            //
            let agent = agent.build();

            println!("=============================== tell me the server version.");
            call_agent_stream(
                &agent,
                emit.clone(),
                Message::user("tell me the server version."),
            )
            .await
            .unwrap();

            println!("=========================================================== ");
            call_agent_stream(
                &agent,
                emit.clone(),
                Message::user("tell me who is the oldest in table Person"),
            )
            .await
            .unwrap();
        }
    }

    #[tokio::test]
    async fn test_read_documents() {
        // 1. Fix path construction: PathBuf is more robust and removes extra quotes.
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
