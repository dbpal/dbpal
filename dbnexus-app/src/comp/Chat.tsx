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
  Stack,
  TextField,
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
import { AITextCompletionName, createChatAPI, editorAppendContent } from "./Common";
import { ModalHeader } from "./CommonComp";
import { message } from "@tauri-apps/plugin-dialog";
import { AllKindQuery } from "./Left";
import { invoke } from "@tauri-apps/api/core";
import types from "./ApiStruct";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import { HumanMessage, AIMessage, BaseMessage } from "@langchain/core/messages";
import { FaArrowAltCircleRight } from "react-icons/fa";
import { FaArrowAltCircleDown } from "react-icons/fa";
import {
  callUnsupported,
  getLocale,
  listCassandraAgg,
  listCreatedTypes,
  listEvent,
  listMaterializedView,
  listProcedure,
  listSequence,
  listUdf,
  showCassandraAgg,
  showProcedure,
  showTableColumn,
  showUdf,
  showViewColumns,
} from "./handlers";
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

class SaveChatDirection {
  static key = "dbpal-chat-direction";

  static get() {
    return localStorage.getItem(SaveChatDirection.key);
  }
  static set(d: string) {
    localStorage.setItem(SaveChatDirection.key, d);
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
  const [chatDirection, setChatDirection] = useState(
    SaveChatDirection.get() || "vertical",
  );

  useEffect(() => {
    SaveChatDirection.set(chatDirection);
  }, [chatDirection]);

  return (
    <Stack
      style={{
        height: "100%",
        width: "100%",
      }}
    >
      <PanelGroup direction={chatDirection as "vertical"} autoSaveId="chat">
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
          <PanelResizeHandle
            style={{
              width: chatDirection != "vertical" ? 4 : undefined,
              height: chatDirection == "vertical" ? 4 : undefined,
            }}
            className="resize-handle"
          />
        )}
        {llmconfig && (
          <Panel collapsible maxSize={45} collapsedSize={0} minSize={8}>
            <SqlChatTextInput
              uuid={uuid}
              kind={kind}
              editor={editor}
              setChatDirection={setChatDirection}
              chatDirection={chatDirection}
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
2. Don't generate markdown.
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
2. Don't generate markdown.
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
    let elements = await this.allSqlElements();

    for (const table of elements.tables) {
      if (!this.question.includes(table.name)) {
        continue;
      }
      let cols = await showTableColumn(this.uuid, table.name);
      ret += `${table.name} is table
        Columns:${cols.map(x => x.field).join(",")}
        Primary key: ${cols
          .filter(x => x.is_primary)
          .map(x => (x.is_auto_increment ? `${x.field}(auto increment)` : x.field))
          .join(",")}\n`;
    }
    for (const view of elements.views) {
      if (!this.question.includes(view.name)) {
        continue;
      }
      let cols = await showViewColumns(this.uuid, view.name);
      ret += `${view.name} is view 
        Columns:${cols.map(x => x.field).join(",")}\n`;
    }
    for (const view of elements.materializedViews) {
      if (!this.question.includes(view.name)) {
        continue;
      }
      let cols = await showViewColumns(this.uuid, view.name);
      ret += `${view.name} is materalized view 
        Columns:${cols.map(x => x.field).join(",")}\n`;
    }

    // udf
    for (const udf of elements.udfs) {
      if (!this.question.includes(udf.name)) {
        continue;
      }
      ret += `${udf.name} is user define function \n`;
      const detail = await showUdf(this.uuid, udf, this.kind);
      ret += `define sql is:\n${detail.def}\n`;
    }
    // cassandra agg
    for (const udf of elements.cassandraAggs) {
      if (!this.question.includes(udf.name)) {
        continue;
      }
      ret += `${udf.name} is user cassandra aggregate function \n`;
      const detail = await showCassandraAgg(
        this.uuid,
        udf.name,
        udf.cassandra_parameters,
      );
      ret += `define sql is:\n${detail.def}\n`;
    }

    for (const udf of elements.procedures) {
      if (!this.question.includes(udf.name)) {
        continue;
      }
      ret += `${udf.name} is procedure \n`;
      const detail = await showProcedure(this.uuid, udf, this.kind);
      ret += `define sql is:\n${detail.def}\n`;
    }

    // types
    for (const ty of elements.types) {
      if (!this.question.includes(ty.name)) {
        continue;
      }
      ret += `${ty.name} is user created type \n`;
      ret += `define sql is:\n${ty.sql}\n`;
    }
    // sequence
    for (const ty of elements.sequences) {
      if (!this.question.includes(ty.name)) {
        continue;
      }
      ret += `${ty.name} is user created sequence \n`;
      ret += `define sql is:\n${ty.sql}\n`;
    }
    // events
    for (const event of elements.events) {
      if (!this.question.includes(event.name)) {
        continue;
      }
      ret += `${event.name} is user created event \n`;
      ret += `define sql is:\n${event.sql}\n`;
    }
    return ret;
  }

  async allSqlElements(): Promise<{
    tables: types.ListTable[];
    views: types.ListTable[];
    materializedViews: types.ListTable[];
    udfs: types.NamedStaff[];
    types: types.CreatedTypeDef[];
    events: types.SqlEvent[];
    sequences: types.SequenceDetail[];
    cassandraAggs: types.NamedStaff[];
    procedures: types.NamedStaff[];
  }> {
    const tables = await callUnsupported(async () => {
      return await invoke<types.ListTable[]>("list_table", { name: this.uuid });
    });

    const views = await callUnsupported(async () => {
      return await invoke<types.ListTable[]>("list_view", { name: this.uuid });
    });
    const materializedViews = await callUnsupported(async () => {
      return listMaterializedView(this.uuid);
    });
    const udfs = await callUnsupported(async () => {
      return await listUdf(this.uuid);
    });

    const events = await callUnsupported(async () => {
      return await listEvent(this.uuid);
    });
    const sequences = await callUnsupported(async () => {
      return await listSequence(this.uuid);
    });

    const procedures = await callUnsupported(async () => {
      return await listProcedure(this.uuid);
    });

    const types = await callUnsupported(async () => {
      return await listCreatedTypes(this.uuid);
    });
    const cassandraAggs = await callUnsupported(async () => {
      return await listCassandraAgg(this.uuid);
    });

    return {
      tables,
      views,
      materializedViews,
      udfs,
      events,
      sequences,
      types,
      procedures,
      cassandraAggs,
    };
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
  setChatDirection;
  chatDirection;
}) {
  const { uuid, kind, editor, setChatDirection, chatDirection } = props;
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
  const [chatBufferSize, setChatBufferSize] = useState(SaveChatMemoryK.get() + "");
  const [showEditPromptTemplate, setShowEditPromptTemplate] = useState(false);
  useEffect(() => {
    const size = parseInt(chatBufferSize);
    if (isNaN(size)) {
      return;
    }
    if (size % 2 != 0) {
      showNotification("warning", "chat buffer size must be even number");
      return;
    }
    SaveChatMemoryK.save(size);
    chatMemory.current.setK(size);
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
      const stream = await chatApi.stream(prompt);
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
    <Stack style={{ height: "100%", width: "100%" }}>
      <Stack direction="row" style={{ alignItems: "flex-end" }}>
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
              setShowChatHistory(e => !e);
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
              setShowPrompt(e => !e);
            }}
            variant="text"
          >
            prompt
          </Button>
        </Tooltip>
        <Tooltip title="edit prompt template">
          <IconButton
            onClick={() => {
              setShowEditPromptTemplate(e => !e);
            }}
          >
            <EditIcon color="secondary"></EditIcon>
          </IconButton>
        </Tooltip>
        <TextField
          value={chatBufferSize}
          error={isNaN(parseInt(chatBufferSize))}
          label="chat memory window size"
          onChange={e => {
            setChatBufferSize(e.target.value);
          }}
          style={{ width: "12rem", flexShrink: 1, minWidth: "4rem" }}
          type="number"
          slotProps={{
            htmlInput: {
              step: 2,
              min: 0,
              max: 20,
            },
          }}
        ></TextField>

        <Tooltip title="chat input location">
          <IconButton
            onClick={() => {
              setChatDirection(e => (e == "vertical" ? "horizontal" : "vertical"));
            }}
          >
            {chatDirection == "vertical" ? (
              <FaArrowAltCircleRight fontSize="1.4rem"> </FaArrowAltCircleRight>
            ) : (
              <FaArrowAltCircleDown fontSize="1.4rem"> </FaArrowAltCircleDown>
            )}
          </IconButton>
        </Tooltip>
      </Stack>
      <Divider> </Divider>
      <SimpleFlexEditor
        language="text"
        handleValue={v => {
          setNewQuestion(v);
        }}
        handleEditor={e => {
          AITextCompletionName.set(e.getModel().id, uuid);
        }}
      ></SimpleFlexEditor>
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
                    editor.focus();
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
        height: "75%",
        overflow: "auto",
      }}
    >
      <Stack height="100%">
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
        <div style={{ flex: 1 }}>
          <pre style={{ height: "100%", overflow: "auto" }}>{prompt}</pre>
        </div>
      </Stack>
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
