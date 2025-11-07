import { ChatOpenAI } from "@langchain/openai";
import {
  appStore,
  DataSourceKind,
  handleCopyToClipboard,
  LLMConfig,
  LLMConfigContext,
  TabActiveContext,
} from "./helper";
import {
  Button,
  Chip,
  Divider,
  IconButton,
  Paper,
  Slider,
  Stack,
  Tooltip,
} from "@mui/material";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { ReactNode, useContext, useEffect, useRef, useState } from "react";
import { editor } from "monaco-editor";
import { AppMarkDown, SimpleFlexEditor } from "./CommonComp2";
import ArrowCircleUpIcon from "@mui/icons-material/ArrowCircleUp";
import EventNoteIcon from "@mui/icons-material/EventNote";
import EditIcon from "@mui/icons-material/Edit";
import { NotificationContext } from "./Notification";
import { PromptTemplate } from "@langchain/core/prompts";

import {
  AITextCompletionName,
  createChatAPI,
  editorAppendContent,
  getLocale,
  showTableColumn,
  showViewColumns,
} from "./Common";
import { ModalHeader } from "./CommonComp";
import { message } from "@tauri-apps/plugin-dialog";
import { AllKindQuery } from "./Left";
import { invoke } from "@tauri-apps/api/core";
import types from "./ApiStruct";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import { HumanMessage, AIMessage, BaseMessage } from "@langchain/core/messages";

class SimpleArrayMemory {
  history: BaseMessage[];
  k: number;
  constructor(k = 4) {
    this.history = [];
    this.k = k;
  }

  addUserMessage(message) {
    this.history.push(new HumanMessage(message));
    this._trimHistory(); // 添加后立即修剪
  }

  addAIMessage(message) {
    this.history.push(new AIMessage(message));
    this._trimHistory(); // 添加后立即修剪
  }

  // 内部修剪：删除最旧的消息，保留最近 k 条
  _trimHistory() {
    if (this.history.length > this.k) {
      // 从头部删除多余的消息
      this.history.splice(0, this.history.length - this.k);
    }
  }

  setK(k: number) {
    this.k = k;
    this._trimHistory();
  }

  // 直接返回数组（已确保最多只有 k 条）
  getMessages() {
    return this.history;
  }

  // 清空历史
  clear() {
    this.history = [];
  }
}

function Chat(props: {
  uuid: string;
  kind: DataSourceKind;
  value?: string;
  favoriteName?: string;
}) {
  const { uuid, kind, favoriteName, value } = props;
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const llmconfig = useContext(LLMConfigContext);
  return (
    <Stack
      style={{
        height: "100%",
        width: "100%",
      }}
    >
      <PanelGroup direction="vertical" autoSaveId="chat">
        <Panel minSize={10} maxSize={100} collapsible collapsedSize={0}>
          <AllKindQuery
            uuid={uuid}
            kind={kind}
            handleEditor={e => {
              setEditor(e);
            }}
            favoriteName={favoriteName}
            value={value}
          ></AllKindQuery>
        </Panel>
        {llmconfig && (
          <PanelResizeHandle style={{ height: 4 }} className="resize-handle" />
        )}
        {llmconfig && (
          <Panel collapsible collapsedSize={0} minSize={8}>
            <SqlChatTextInput
              uuid={uuid}
              kind={kind}
              editor={editor}
            ></SqlChatTextInput>
          </Panel>
        )}
      </PanelGroup>
    </Stack>
  );
}

class MakePrompt {
  static getDefaultTemplatePredefineVariables(kind: DataSourceKind) {
    switch (kind) {
      case DataSourceKind.Redis:
        return MakePrompt.redisTemplate.inputVariables;

      default:
        return MakePrompt.sqlTemplate.inputVariables;
    }
  }

  static getDefaultTemplate(kind: DataSourceKind): string {
    switch (kind) {
      case DataSourceKind.Redis:
        return MakePrompt.redisTemplate.template as string;
      default:
        return MakePrompt.sqlTemplate.template as string;
    }
  }

  static sqlTemplate = PromptTemplate.fromTemplate(
    `system:You are a {dialect} database expert.
Your answer must follow ALL of these rules:
1. Return SQL only.
2. Do NOT wrap code in \`\`\`markdown.
3. Comments must use "--" syntax.
4. If you break any rule, you will be penalized.
5. system locale is {locale} 


human: here are some information about the database
{database_information}

Chat History:
{chat_history}

human: {question}`,
  );

  static redisTemplate = PromptTemplate.fromTemplate(
    `system:You are a Redis expert.
Your answer must follow ALL of these rules:
1. Return Command only.
2. Do NOT wrap code in \`\`\`markdown.
3. Comments must use "#" syntax.
4. If you break any rule, you will be penalized.
5. system locale is {locale}

human: here are some information about the database
{database_information}


Chat History:
{chat_history}

human:{question}
`,
  );

  constructor(
    public uuid: string,
    public config: LLMConfig,
    public kind: DataSourceKind,
    public question: string,
    public memory: SimpleArrayMemory,
  ) {}

  kind2CommonName(): string {
    switch (this.kind) {
      case DataSourceKind.SqlServer:
        return "sql server(microsoft)";
      default:
        return this.kind as string;
    }
  }

  async redisDatabaseInformation() {
    const typeHints = await invoke<object>("get_redis_type_hint", {
      name: this.uuid,
      text: this.question.trim(),
    });
    let ret = "";
    for (const key in typeHints) {
      ret += `  ${key} has type '${typeHints[key]}'\n`;
    }
    return ret;
  }

  async createRedisPrompt(): Promise<string> {
    let inputs = {
      question: this.question.trim(),
      database_information: await this.redisDatabaseInformation(),
      chat_history: this.memory
        .getMessages()
        .map(x => x.toFormattedString())
        .join("\n"),
      locale: await getLocale(),
    };
    let userDefine = this.config.aiTextTemplates[this.kind];
    if (userDefine) {
      return PromptTemplate.fromTemplate(userDefine).format(inputs);
    }
    // default template
    return MakePrompt.redisTemplate.format(inputs);
  }
  async createSqlPrompt(): Promise<string> {
    let inputs = {
      dialect: this.kind2CommonName(),
      question: this.question,
      database_information: await this.sqlDatabaseInformation(),
      chat_history:
        this.memory
          .getMessages()
          .map(x => x.toFormattedString())
          .join("\n") + "\n",
      locale: await getLocale(),
    };
    const userDefine = this.config.aiTextTemplates[this.kind];

    if (userDefine) {
      return PromptTemplate.fromTemplate(userDefine).format(inputs);
    }
    // default template
    return MakePrompt.sqlTemplate.format(inputs);
  }

  async sqlDatabaseInformation() {
    let ret = "";
    let tables = await this.allSqlTables();

    for (const table of tables.tables) {
      if (!this.question.includes(table.name)) {
        continue;
      }
      let cols = await showTableColumn(this.uuid, table.name);
      ret += `${table.name} is table
        Columns:${cols.map(x => x.field).join(",")}
        Primary key: ${cols
          .filter(x => x.is_primary)
          .map(x => (x.is_auto_increment ? `${x.field}(auto increment)` : x.field))
          .join(",")} 

        `;
    }
    for (const view of tables.views) {
      if (!this.question.includes(view.name)) {
        continue;
      }
      let cols = await showViewColumns(this.uuid, view.name);
      ret += `${view.name} is view 
        Columns:${cols.map(x => x.field).join(",")}
        `;
    }
    for (const view of tables.views) {
      if (!this.question.includes(view.name)) {
        continue;
      }
      let cols = await showViewColumns(this.uuid, view.name);
      ret += `${view.name} is materalized view 
        Columns:${cols.map(x => x.field).join(",")}
        `;
    }

    return ret;
  }

  async allSqlTables(): Promise<{
    tables: types.ListTable[];
    views: types.ListTable[];
    materializedViews: types.ListTable[];
  }> {
    let tables = [];
    try {
      tables = await invoke<types.ListTable[]>("list_table", { name: this.uuid });
    } catch (e) {
      console.log("list table failed,this maybe a error", e);
    }

    let views = [];
    try {
      views = await invoke<types.ListTable[]>("list_view", { name: this.uuid });
    } catch (e) {
      console.log("list view failed,this maybe a error", e);
    }
    let materializedViews = [];
    try {
      materializedViews = await invoke<types.ListTable[]>("list_materialized_view", {
        name: this.uuid,
      });
    } catch (e) {
      console.log("list materialized view failed,this maybe a error", e);
    }
    return { tables, views, materializedViews };
  }
  async createPrompt(): Promise<string> {
    switch (this.kind) {
      case DataSourceKind.Redis:
        return this.createRedisPrompt();
      default:
        return this.createSqlPrompt();
    }
  }
}

class SaveChatMemoryK {
  static key = "dbpal-chat-memory-k";
  static defaultK = 4;

  static get(): number {
    try {
      const x = localStorage.getItem(SaveChatMemoryK.key);
      return x ? parseInt(x) : SaveChatMemoryK.defaultK;
    } catch (e) {
      return SaveChatMemoryK.defaultK;
    }
  }

  static save(x: number) {
    localStorage.setItem(SaveChatMemoryK.key, x + "");
  }
}

function SqlChatTextInput(props: {
  uuid: string;
  kind: DataSourceKind;
  editor: editor.IStandaloneCodeEditor;
}) {
  const { uuid, kind, editor } = props;
  const config = useContext(LLMConfigContext);
  const [chatApi, setChatApi] = useState<ChatOpenAI>(null);
  const [prevQuestion, setPrevQuestion] = useState("");
  const [newQuestion, setNewQuestion] = useState("");
  const { showNotification } = useContext(NotificationContext);
  const [showPrompt, setShowPrompt] = useState(false);
  useEffect(() => {
    console.log("create llm because of config change");
    setChatApi(createChatAPI(config));
  }, [config]);

  const [loading, setLoading] = useState(false);
  const [chatHistory, setChatHistory] = useState<string[]>([]);
  const [showChatHistory, setShowChatHistory] = useState(false);
  const chatMemory = useRef(new SimpleArrayMemory());
  const [chatBufferSize, setChatBufferSize] = useState(SaveChatMemoryK.get());
  const [showEditPromptTemplate, setShowEditPromptTemplate] = useState(false);
  useEffect(() => {
    SaveChatMemoryK.save(chatBufferSize);
    chatMemory.current.setK(chatBufferSize);
  }, [chatBufferSize]);

  const handleChat = async () => {
    try {
      setLoading(true);
      editor.setValue("");
      setPrevQuestion(newQuestion);
      const prompt = await new MakePrompt(
        uuid,
        config,
        kind,
        newQuestion,
        chatMemory.current,
      ).createPrompt();
      chatMemory.current.addUserMessage(newQuestion);
      setChatHistory(e => [...e, newQuestion]);
      let wholeText = "";
      console.time("stream-first-chunk");
      const stream = await chatApi.stream(prompt, {});

      console.timeEnd("stream-first-chunk");
      for await (const msg of stream) {
        editorAppendContent(editor, msg.content);
        wholeText += msg.content;
      }
      setChatHistory(e => [...e, wholeText]);
      chatMemory.current.addAIMessage(wholeText);
    } catch (e) {
      showNotification("error", "call llm failed", e);
    } finally {
      setLoading(false);
    }
  };

  return (
    <Stack style={{ height: "100%" }} direction="row">
      <SimpleFlexEditor
        language="text"
        handleValue={v => {
          setNewQuestion(v);
        }}
        disableEditorBar
        handleEditor={e => {
          AITextCompletionName.set(e.getModel().id, uuid);
        }}
      ></SimpleFlexEditor>
      <Stack style={{ width: "fit-content", overflow: "auto", alignItems: "center" }}>
        <IconButton
          onClick={handleChat}
          disabled={newQuestion == prevQuestion}
          loading={loading}
        >
          <ArrowCircleUpIcon
            style={{ fontSize: "1.8rem" }}
            color={newQuestion == prevQuestion ? "disabled" : "primary"}
          ></ArrowCircleUpIcon>
        </IconButton>
        <Tooltip title="history">
          <IconButton
            onClick={() => {
              setShowChatHistory(true);
            }}
            disabled={chatHistory.length == 0}
          >
            <EventNoteIcon
              color={chatHistory.length == 0 ? "disabled" : "primary"}
            ></EventNoteIcon>
          </IconButton>
        </Tooltip>
        <Tooltip title="show prompt">
          <Button
            disabled={newQuestion.trim() == ""}
            onClick={() => {
              setShowPrompt(true);
            }}
            variant="text"
          >
            prompt
          </Button>
        </Tooltip>
        <Tooltip title="edit prompt template">
          <IconButton
            onClick={() => {
              setShowEditPromptTemplate(true);
            }}
          >
            <EditIcon color="secondary"></EditIcon>
          </IconButton>
        </Tooltip>
        <Tooltip title={`chat history buffer size(${chatBufferSize})`}>
          <Slider
            value={chatBufferSize}
            onChange={(_, v) => {
              setChatBufferSize(v as number);
            }}
            orientation="vertical"
            min={0}
            max={20}
            step={2}
            style={{
              flexShrink: 1,
              minHeight: "3rem",
              maxHeight: "10rem",
            }}
          ></Slider>
        </Tooltip>
      </Stack>

      {showChatHistory && (
        <ChatHistory
          history={chatHistory}
          handleClose={() => {
            setShowChatHistory(false);
          }}
          handleClean={() => {
            setChatHistory([]);
          }}
        ></ChatHistory>
      )}
      {showEditPromptTemplate && (
        <EditPromptTemplate
          config={config}
          handleClose={() => {
            setShowEditPromptTemplate(false);
          }}
          kind={kind}
        ></EditPromptTemplate>
      )}
      {showPrompt && (
        <ShowPrompt
          chatMemory={chatMemory.current}
          uuid={uuid}
          kind={kind}
          question={newQuestion}
          config={config}
          handleClose={() => {
            setShowPrompt(false);
          }}
        ></ShowPrompt>
      )}
    </Stack>
  );
}

function EditPromptTemplate(props: {
  kind: DataSourceKind;
  config: LLMConfig;
  handleClose: () => void;
}) {
  const { kind, handleClose, config } = props;
  const userDefine = config.aiTextTemplates[kind];
  const [newTemplate, setNewTemplate] = useState(
    userDefine ? userDefine : MakePrompt.getDefaultTemplate(kind),
  );
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const { showNotification } = useContext(NotificationContext);
  const handleSave = async () => {
    try {
      if (!config.aiTextTemplates) {
        config.aiTextTemplates = {};
      }
      config.aiTextTemplates[kind] = newTemplate;
      await config.save();
    } catch (e) {
      showNotification("error", "save template failed", e);
      return;
    }
    showNotification("success", "save template ok");
    handleClose();
  };

  const handleDelete = async () => {
    try {
      config.aiTextTemplates[kind] = "";
      await config.save();
    } catch (e) {
      showNotification("error", "reset template failed", e);
      return;
    }
    showNotification("success", "reset template ok");
    handleClose();
  };

  const vis = useContext(TabActiveContext);
  useEffect(() => {
    const closeThis = (e: KeyboardEvent) => {
      if (e.key === "Escape" && vis) {
        handleClose();
      }
    };
    document.addEventListener("keydown", closeThis);
    return () => {
      document.removeEventListener("keydown", closeThis);
    };
  }, [vis, handleClose]);

  return (
    <Paper
      style={{
        position: "absolute",
        width: "80%",
        height: "80%",
        bottom: 0,
        right: 0,
        zIndex: 14,
        display: "flex",
        flexDirection: "column",
      }}
    >
      <Stack style={{ height: "100%" }}>
        <ModalHeader title="edit template" handleClose={handleClose}></ModalHeader>
        <SimpleFlexEditor
          value={newTemplate}
          handleValue={v => {
            setNewTemplate(v);
          }}
          handleEditor={e => {
            setEditor(e);
          }}
          language="text"
        ></SimpleFlexEditor>
        <Divider></Divider>
        <Stack direction="row" style={{ alignItems: "center" }}>
          <Button onClick={handleSave}> save </Button>
          {MakePrompt.getDefaultTemplatePredefineVariables(kind).map(variable => {
            return (
              <Tooltip title={`click to use this variable in template.`}>
                <Chip
                  label={variable as string}
                  onClick={() => {
                    editor
                      .getModel()
                      .pushEditOperations(
                        null,
                        [{ range: editor.getSelection(), text: `{${variable}}` }],
                        null,
                      );
                  }}
                ></Chip>
              </Tooltip>
            );
          })}
          <Tooltip title="delete template,after we will use system default template">
            <IconButton onClick={handleDelete}>
              <DeleteForeverIcon> </DeleteForeverIcon>
            </IconButton>
          </Tooltip>
        </Stack>
      </Stack>
    </Paper>
  );
}

function ChatHistory(props: {
  history: string[];
  handleClose: () => void;
  handleClean;
}) {
  const { history, handleClose, handleClean } = props;
  const vis = useContext(TabActiveContext);
  useEffect(() => {
    const closeThis = (e: KeyboardEvent) => {
      if (e.key === "Escape" && vis) {
        handleClose();
      }
    };
    document.addEventListener("keydown", closeThis);
    return () => {
      document.removeEventListener("keydown", closeThis);
    };
  }, [vis, handleClose]);
  const wrapperRef = useRef(null);
  useEffect(() => {
    const el = wrapperRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, []);
  const handleClear = () => {
    handleClean();
    handleClose();
  };

  return (
    <Paper
      style={{
        position: "absolute",
        bottom: 0,
        right: 0,
        width: "87%",
        height: "87%",
        zIndex: 14,
        overflow: "auto",
        padding: "0.4rem",
      }}
      ref={wrapperRef}
    >
      <ModalHeader
        title="chat history"
        headerExtra={
          <Tooltip title="delete history">
            <IconButton onClick={handleClear}>
              <DeleteForeverIcon> </DeleteForeverIcon>
            </IconButton>
          </Tooltip>
        }
        handleClose={handleClose}
      ></ModalHeader>
      <Stack>
        {history.map((his, index) => {
          return (
            <div key={index}>
              <Stack
                direction="row"
                style={{
                  justifyContent: index % 2 == 0 ? "flex-start" : "flex-end",
                }}
              >
                <div
                  style={{
                    width: "85%",
                  }}
                >
                  <AppMarkDown
                    doc={his}
                    handlePathUrl={href => {
                      message(`sorry we don't know how to go to this path ${href}.`, {
                        kind: "error",
                      });
                    }}
                  ></AppMarkDown>
                </div>
              </Stack>
              {index > 0 && index % 2 == 1 && <Divider />}
            </div>
          );
        })}
      </Stack>
    </Paper>
  );
}

function ShowPrompt(props: {
  uuid: string;
  kind: DataSourceKind;
  question: string;
  config: LLMConfig;
  handleClose: () => void;
  chatMemory: SimpleArrayMemory;
}) {
  const { uuid, kind, question, config, handleClose, chatMemory } = props;
  const [prompt, setPrompt] = useState("");
  const vis = useContext(TabActiveContext);
  const { showNotification } = useContext(NotificationContext);

  useEffect(() => {
    const closeThis = (e: KeyboardEvent) => {
      if (e.key === "Escape" && vis) {
        handleClose();
      }
    };
    document.addEventListener("keydown", closeThis);
    return () => {
      document.removeEventListener("keydown", closeThis);
    };
  }, [vis, handleClose]);

  useEffect(() => {
    new MakePrompt(uuid, config, kind, question, chatMemory).createPrompt().then(e => {
      setPrompt(e);
    });
  }, [question]);
  return (
    <Paper
      style={{
        position: "absolute",
        right: 0,
        bottom: 0,
        zIndex: 14,
        padding: "0.5rem",
        width: "75%",
        overflow: "auto",
      }}
    >
      <ModalHeader
        headerExtra=<>
          <Button
            variant="text"
            onClick={() => {
              handleCopyToClipboard(prompt, showNotification);
            }}
          >
            copy
          </Button>
        </>
        title="show prompt"
        handleClose={handleClose}
      ></ModalHeader>
      <pre> {prompt} </pre>
    </Paper>
  );
}

function AllKindChat(props: {
  uuid: string;
  kind: DataSourceKind;
  value?: string;
  favoriteName?: string;
}) {
  const [config, setConfig] = useState<LLMConfig>(null);
  const { showNotification } = useContext(NotificationContext);

  const handleReload = () => {
    LLMConfig.get()
      .then(config => {
        if (config && config.enable) {
          setConfig(config);
        }
      })
      .catch(e => {
        showNotification("error", "get llm config failed", e);
      });
  };
  useEffect(() => {
    const unlisten = appStore.onKeyChange(LLMConfig.key, () => {
      handleReload();
    });
    return () => {
      unlisten.then(x => {
        x();
      });
    };
  }, []);

  useEffect(() => {
    handleReload();
  }, []);
  let chat: ReactNode;
  switch (props.kind) {
    default:
      chat = <Chat {...props}></Chat>;
  }
  return (
    config && (
      <LLMConfigContext.Provider value={config}> {chat} </LLMConfigContext.Provider>
    )
  );
}

export { AllKindChat };
