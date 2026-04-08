use base64::Engine;

use dbnexus_db::db::{BackendKind, ExportFrame, ExportKind};
use dbnexus_db::mongo::MongoDB;

use dbnexus_db::redis::RedisDB;
use futures_util::StreamExt;
use mongodb::bson::doc;
use rig::agent::{Agent, MultiTurnStreamItem, PromptHook, StreamingError, Text};
use rig::client::completion::CompletionClient;
use rig::providers::openai::responses_api::ResponsesCompletionModel;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};
use rig::tool::ToolDyn;
use rig::wasm_compat::{WasmCompatSend, WasmCompatSync};
use rig::EmptyListError;
use serde::{Deserialize, Serialize};
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
    agent_builder = agent_builder.default_max_turns(10);

    let agent = agent_builder.tools(tools).build();
    anyhow::Ok(agent)
}

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
    emit: Emitter,
    comment_fn: impl Fn(&str) -> String + Send + Sync + 'static,
    prompt: Message,
) -> Result<(), ErrorFrontend> {
    #[cfg(debug_assertions)]
    {
        println!("prompt:{:?}", prompt);
    }

    let mut prompt = agent.stream_prompt(prompt).await;
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
                    emit(comment_fn(
                        format!(
                            "\ninput token:{} output token:{} total token:{}",
                            x.usage.input_tokens, x.usage.output_tokens, x.usage.total_tokens,
                        )
                        .as_str(),
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
                            ToolResultContent::Text(text) => {
                                emit(comment_fn(if let Some(x) = text.text().get(0..20) {
                                    x
                                } else {
                                    text.text()
                                }))
                            }
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

impl DBPal {
    async fn make_tools(
        &self,
        name: &str,
        query_id: impl Into<String>,
        emit: Emitter,
    ) -> anyhow::Result<Vec<Box<dyn ToolDyn>>> {
        let db = self.get_backend(&name).await?;
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
                let r: Vec<Box<dyn ToolDyn>> = vec![Box::new(mongo::MongoQueryTool {
                    client: c,
                    max_limit: 10_000,
                })];
                anyhow::Ok(r)
            }

            _ => {
                // sql base database
                let r: Vec<Box<dyn ToolDyn>> = vec![Box::new(SqlQueryTool {
                    db: db,
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

impl From<mongodb::error::Error> for CallToolErr {
    fn from(value: mongodb::error::Error) -> Self {
        Self {
            err_msg: value.to_string(),
        }
    }
}
impl From<String> for CallToolErr {
    fn from(value: String) -> Self {
        Self {
            err_msg: value.into(),
        }
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
                .comment(format!("\ncall tool sql query {}", args.query).as_str()),
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
    let emitter = Arc::new(move |s| {
        if let Err(err) = emit.send(s) {
            log::error!("Error sending message: {:?}", err)
        }
    });
    let mut tools = dbpal
        .make_tools(&name, query_id, emitter.clone())
        .await
        .map_err(ErrorFrontend::from)?;

    if let Some(x) = req.search_engine.make_tool(emitter.clone())? {
        tools.push(x);
    }
    let db = dbpal
        .get_backend(&name)
        .await
        .map_err(ErrorFrontend::from)?;
    let db_kind = db.backend_kind();
    let agent = make_agent(
        &req,
        tools,
        Some(format!("U a are {} expert", db.backend_kind().to_string()).as_str()),
    )
    .unwrap();

    let msg = Message::User {
        content: {
            let mut content = vec![];

            content.push(UserContent::text(format!(
                "chat_history:\n{}\n",
                req.chat_history
                    .iter()
                    .map(|x| format!("{}:{}\n", x.role, x.msg,))
                    .collect::<String>()
            )));
            if db.backend_kind() == BackendKind::Mongo {
                content.push(UserContent::text(make_mongo_prompt()));
            }
            content.push(UserContent::text(format!(
                "system local is :{}",
                sys_locale::get_locale().unwrap_or("en-us".to_string())
            )));
            content.push(UserContent::Text(Text {
                text: req.prompt.clone(),
            }));

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

    let fur = call_openai_stream(agent, emitter, move |x| db_kind.comment(x), msg);
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
    emit: Emitter,
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
            .map_err(|err| CallToolErr::from(format!("call search engine failed,err:{}", err)))?;

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

mod mongo {

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
            let db = self.client.get_db().await.unwrap();

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
            let limit = limit.unwrap_or(50).min(self.max_limit);

            let result = match operation.as_str() {
                "find" => {
                    let mut find = collection.find(filter.unwrap_or_default());

                    if let Some(proj) = projection {
                        find = find.projection(proj);
                    }
                    if let Some(sort) = sort {
                        find = find.sort(sort);
                    }
                    find = find.limit(limit);
                    if let Some(skip) = skip {
                        find = find.skip(skip);
                    }
                    let mut cursor = find.await?;
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
                        let x = x.unwrap();
                        docs.push(x);
                    }
                    docs
                }

                "count" => {
                    let count = collection
                        .count_documents(filter.unwrap_or_default())
                        .await
                        .unwrap();
                    vec![doc! { "count": count as i64 }]
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
        r#"
const db : IMongoDB; // db is a database handle can access the database
const setUpClean : (name :string, f : ()=>void)=> void; // setup a clean function by name and callback
const bson : any; // bson will initialize by 'await import("bson")'
        "#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbnexus_db::{
        datasource_config::SqliteDBConfig, db::QuerySql, enable_test_log, sqlite::SqliteDB,
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
        // enable_test_log();
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
                chat_history : vec![],
                prompt: r#"


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

        let r = agent.prompt(&req.prompt).await.unwrap();

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
            chat_history : vec![],
                url: "https://api.openai-proxy.org/v1".into(),
                api_key: "sk-egR0EUYn0EzVybl7D9JkUFx2ksnD62sZPQ8d0J0yJgPEiAWt".into(),
                model: "gpt-4.1-mini".into(),
                prompt: r#"


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
            chat_history: vec![],
            url: "https://api.openai-proxy.org/v1".into(),
            api_key: "sk-egR0EUYn0EzVybl7D9JkUFx2ksnD62sZPQ8d0J0yJgPEiAWt".into(),
            model: "gpt-4.1-mini".into(),
            prompt: r#"
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

        let agent = make_agent(
            req,
            vec![Box::new(WebSearchTool {
                engine: Box::new(websearch::providers::DuckDuckGoProvider::new()),
                emit: p.clone(),
            })],
            None,
        )
        .unwrap();

        let msg = Message::user(&req.prompt);

        call_openai_stream(agent, p, |x| x.to_owned(), msg)
            .await
            .unwrap();
    }
}
