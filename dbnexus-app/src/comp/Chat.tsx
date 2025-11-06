import { ChatOpenAI } from "@langchain/openai";
import {
  appStore,
  DataSourceKind,
  LLMConfig,
  LLMConfigContext,
  TabActiveContext,
} from "./helper";
import { Button, Divider, IconButton, Paper, Stack, Tooltip } from "@mui/material";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { ReactNode, useCallback, useContext, useEffect, useRef, useState } from "react";
import { editor } from "monaco-editor";
import { AppMarkDown, SimpleFlexEditor } from "./CommonComp2";
import ArrowCircleUpIcon from "@mui/icons-material/ArrowCircleUp";
import EventNoteIcon from "@mui/icons-material/EventNote";
import EditIcon from "@mui/icons-material/Edit";
import { NotificationContext } from "./Notification";
import { PromptTemplate } from "@langchain/core/prompts";
import {
  AITextCompletionName,
  createLLM,
  editorAppendContent,
  showTableColumn,
  showViewColumns,
} from "./Common";
import { ModalHeader } from "./CommonComp";
import { message } from "@tauri-apps/plugin-dialog";
import { AllKindQuery } from "./Left";
import { invoke } from "@tauri-apps/api/core";
import types from "./ApiStruct";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
function SqlChat(props: {
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

const sqlTemplate = PromptTemplate.fromTemplate(`You are a {database} database expert.
Your answer must follow ALL of these rules:
1. Return SQL only.
2. Do NOT wrap code in \`\`\`markdown.
3. Comments must use "--" syntax.
4. If you break any rule, you will be penalized.

human: here are some information about the database:
{database_information}


human:{question}`);

class MakePrompt {
  constructor(
    public uuid: string,
    public config: LLMConfig,
    public kind: DataSourceKind,
    public question: string,
  ) {}

  async createSqlPrompt(): Promise<string> {
    let inputs = {
      database: this.kind as string,
      question: this.question,
      database_information: await this.sqlDatabaseInformation(),
    };
    return sqlTemplate.format(inputs);
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
          .map(x => x.field)
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
      console.log("list meterialize view failed,this maybe a error", e);
    }
    return { tables, views, materializedViews };
  }

  async createPrompt(): Promise<string> {
    switch (this.kind) {
      default:
        return this.createSqlPrompt();
    }
  }
}

function SqlChatTextInput(props: {
  uuid: string;
  kind: DataSourceKind;
  editor: editor.IStandaloneCodeEditor;
}) {
  const { uuid, kind, editor } = props;
  const config = useContext(LLMConfigContext);
  const [llm, setLLM] = useState<ChatOpenAI>(null);
  const [prevQuestion, setPrevQuestion] = useState("");
  const [newQuestion, setNewQuestion] = useState("");
  const { showNotification } = useContext(NotificationContext);
  const [showPrompt, setShowPrompt] = useState(false);
  useEffect(() => {
    console.log("create llv because of config change");
    setLLM(createLLM(config));
  }, [config]);
  const [loading, setLoading] = useState(false);
  const [chatHistory, setChatHistory] = useState<string[]>([]);
  const [showChatHistory, setShowChatHistory] = useState(false);
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
      ).createPrompt();
      setChatHistory(e => [...e, newQuestion]);
      let wholeText = "";
      console.time("stream-first-chunk");
      const stream = await llm.stream(prompt, {});
      console.timeEnd("stream-first-chunk");
      for await (const msg of stream) {
        editorAppendContent(editor, msg.content);
        wholeText += msg.content;
      }
      setChatHistory(e => [...e, wholeText]);
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
      <Stack style={{ width: "fit-content", overflow: "auto" }}>
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
          <IconButton>
            <EditIcon color="secondary"></EditIcon>
          </IconButton>
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
      {showPrompt && (
        <ShowPrompt
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

function ChatHistory(props: {
  history: string[];
  handleClose: () => void;
  handleClean;
}) {
  const { history, handleClose, handleClean } = props;
  const vis = useContext(TabActiveContext);
  const keyEscCancelQueryLog = useCallback(
    e => {
      if (e.key === "Escape" && vis) {
        handleClose();
      }
    },
    [vis, handleClose],
  );
  useEffect(() => {
    document.addEventListener("keydown", keyEscCancelQueryLog);
    return () => {
      document.removeEventListener("keydown", keyEscCancelQueryLog);
    };
  }, [keyEscCancelQueryLog]);
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
}) {
  const { uuid, kind, question, config, handleClose } = props;
  const [prompt, setPrompt] = useState("");
  const vis = useContext(TabActiveContext);
  const closeThis = useCallback(
    e => {
      if (e.key === "Escape" && vis) {
        handleClose();
      }
    },
    [vis, handleClose],
  );

  useEffect(() => {
    document.addEventListener("keydown", closeThis);
    return () => {
      document.removeEventListener("keydown", closeThis);
    };
  }, [closeThis]);

  useEffect(() => {
    new MakePrompt(uuid, config, kind, question).createPrompt().then(e => {
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
      <ModalHeader title="show prompt" handleClose={handleClose}></ModalHeader>
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
      chat = <SqlChat {...props}></SqlChat>;
  }
  return (
    config && (
      <LLMConfigContext.Provider value={config}> {chat} </LLMConfigContext.Provider>
    )
  );
}

export { AllKindChat };
