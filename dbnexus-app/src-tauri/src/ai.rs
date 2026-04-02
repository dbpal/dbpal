use std::future::Future;
use std::sync::Arc;

use base64::Engine;
use dbnexus_db::db::{BackendKind, ExportFrame, ExportKind};

use rig::agent::{Agent, PromptHook, StreamingError, Text};
use rig::client::completion::CompletionClient;
use rig::providers::openai::responses_api::ResponsesCompletionModel;
use rig::tool::ToolDyn;
use rig::wasm_compat::{WasmCompatSend, WasmCompatSync};
use rig::EmptyListError;

use crate::app::{self, DBPal, DatabaseBackend, ErrorFrontend, OpenAIReq, TaskManager};
use rig::completion::{CompletionModel, Message, Prompt, PromptError, ToolDefinition};
use rig::message::{DocumentSourceKind, Image, UserContent};
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
    tools: Option<Vec<Box<dyn ToolDyn>>>,
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
    let agent = if let Some(tools) = tools {
        agent_builder.tools(tools).build()
    } else {
        agent_builder.build()
    };
    anyhow::Ok(agent)
}

#[tauri::command]
pub async fn openai_invoke(req: app::OpenAIReq) -> Result<String, ErrorFrontend> {
    let api = make_openai(&req, None).map_err(ErrorFrontend::from)?;
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
    tools: Option<Vec<Box<dyn ToolDyn>>>,
    emit: Emitter,
) -> Result<(), ErrorFrontend> {
    let agent = make_openai(&req, tools)?;
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
    let prompt = agent.prompt(msg);

    let r = prompt.await?;
    emit(r);
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
                todo!()
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

// // 工具参数定义
// #[derive(Debug, serde::Serialize, serde:: Deserialize)]
// struct SqlQueryArgs {
//     query: String,
// }
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

    type Args = SqlQueryArgs;
    type Output = SqlQueryOutput;
    type Error = CallToolErr;

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        (self.emit)(format!("--call tool sql query '{}'\n", args.query));

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
            .map_err(|e| CallToolErr::from(format!("查询失败: {}", e)))
            .unwrap();
        if exceed {
            (self.emit)(format!("-- data exceed 100 some data not feed to LLM"))
        }
        let r = expect_export_frame_all_json(ret).unwrap();
        Ok(SqlQueryOutput {
            result: serde_json::json!({"data": r}),
            query: args.query,
        })
    }
}

fn expect_export_frame_all_json(x: Vec<ExportFrame>) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut ret = vec![];
    for x in x {
        match x {
            ExportFrame::JsonValue(x) => {
                ret.push(x);
            }
            _ => {
                anyhow::bail!(format!("expect json line but:{}", x));
            }
        }
    }
    anyhow::Ok(ret)
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
        emit.send(s).unwrap();
    });
    let fur = call_openai_stream(
        &req,
        Some(db.make_tools(query_id, emitter.clone())),
        emitter,
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

#[cfg(test)]
mod tests {
    use super::*;
    use dbnexus_db::{datasource_config::SqliteDBConfig, db::QuerySql, sqlite::SqliteDB};
    use futures_util::StreamExt;
    use rig::{
        agent::MultiTurnStreamItem,
        streaming::{StreamedAssistantContent, StreamingPrompt},
    };

    #[tokio::test]

    async fn test_xxx() {
        let image = image_to_base64("/home/yuyang/Pictures/屏幕截图/屏幕截图_20260402_070833.png")
            .await
            .unwrap();

        call_openai_stream(
            &OpenAIReq {
                url: "https://api.openai-proxy.org/v1".into(),
                api_key: "sk-egR0EUYn0EzVybl7D9JkUFx2ksnD62sZPQ8d0J0yJgPEiAWt".into(),
                model: "gpt-4.1-mini".into(),
                msg: "this image about?".into(),
                temperature: None,
                max_tokens: None,
                top_p: None,
                images: vec![image],
            },
            None,
            Arc::new(|x| print!("{}", x)),
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
                model: "gpt-5".into(),
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
                model: "gpt-5".into(),
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
                        rig::streaming::StreamedUserContent::ToolResult { .. } => {}
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
}
