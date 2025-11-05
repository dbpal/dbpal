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
import { SqlQuery } from "./SqlQuery";
import { ReactNode, useCallback, useContext, useEffect, useState } from "react";
import { editor } from "monaco-editor";
import { AppMarkDown, SimpleFlexEditor } from "./CommonComp2";
import ArrowCircleUpIcon from "@mui/icons-material/ArrowCircleUp";
import EventNoteIcon from "@mui/icons-material/EventNote";
import EditIcon from "@mui/icons-material/Edit";
import { NotificationContext } from "./Notification";
import { ChatPromptTemplate, PromptTemplate } from "@langchain/core/prompts";
import { editorAppendContent, NameAndQueryId, SetAutoCompletionName } from "./Common";
import { ModalHeader } from "./CommonComp";
import { message } from "@tauri-apps/plugin-dialog";

function createLLM(config: LLMConfig) {
  console.log("createLLM", config);
  const llm = new ChatOpenAI({
    apiKey: config.apiKey,
    temperature: config.temperature,
    modelName: config.model,
    configuration: {
      baseURL: config.baseUrl,
      defaultHeaders: {
        "X-Accel-Buffering": "no",
        "Cache-Control": "no-cache",
      },
    },
    streaming: true,
  });
  return llm;
}

function SqlChat(props: { uuid: string; kind: DataSourceKind }) {
  const { uuid, kind } = props;
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  return (
    <Stack
      style={{
        height: "100%",
        width: "100%",
        position: "relative",
      }}
    >
      <PanelGroup direction="vertical" autoSaveId="chat">
        <Panel minSize={10} maxSize={100} collapsible collapsedSize={0}>
          <SqlQuery
            uuid={uuid}
            kind={kind}
            hasPositionRelative
            handleEditor={e => {
              setEditor(e);
            }}
          ></SqlQuery>
        </Panel>

        <PanelResizeHandle style={{ height: 4 }} className="resize-handle" />
        <Panel collapsible collapsedSize={0} minSize={8}>
          <SqlChatTextInput uuid={uuid} kind={kind} editor={editor}></SqlChatTextInput>
        </Panel>
      </PanelGroup>
    </Stack>
  );
}

const sqlTemplate = ChatPromptTemplate.fromMessages([
  [
    "system",
    `You are a {database} database expert.
Your answer must follow ALL of these rules:
1. Return SQL only.
2. Do NOT wrap code in \`\`\`markdown.
3. Comments must use "--" syntax.
4. If you break any rule, you will be penalized.`,
  ],
  ["human", "{question}"],
]);

//@ts-ignore
async function createPrompt(
  uuid: string,
  kind: DataSourceKind,
  input: string,
  userDefineTemplate: string | "",
): Promise<string> {
  let inputs = { database: kind, question: input };
  if (userDefineTemplate) {
    PromptTemplate.fromTemplate(userDefineTemplate).format(inputs);
  } else {
    return sqlTemplate.format(inputs);
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
      const prompt = await createPrompt(uuid, kind, newQuestion, config.sqlTemplate);
      setChatHistory(e => [...e, prompt]);
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
    }
    setLoading(false);
  };

  return (
    <Stack style={{ height: "100%" }} direction="row">
      <SimpleFlexEditor
        language="sql"
        handleValue={v => {
          setNewQuestion(v);
        }}
        handleEditor={e => {
          SetAutoCompletionName.set(e.getModel().id, new NameAndQueryId(uuid, kind));
        }}
      ></SimpleFlexEditor>
      <Stack style={{ width: "fit-content" }}>
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

function ChatHistory(props: { history: string[]; handleClose: () => void }) {
  const { history, handleClose } = props;
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
  return (
    <Paper
      style={{
        position: "absolute",
        bottom: 0,
        right: 0,
        width: "85%",
        height: "85%",
        zIndex: 14,
        overflow: "auto",
        padding: "0.4rem",
      }}
    >
      <ModalHeader title="chat history" handleClose={handleClose}></ModalHeader>
      <Stack>
        {history.map((his, index) => {
          return (
            <>
              <Stack
                direction="row"
                style={{
                  justifyContent: index % 2 == 0 ? "flex-start" : "flex-end",
                }}
              >
                <div
                  style={{
                    width: "80%",
                    border: "2px solid " + (index % 2 == 0 ? "green" : "skyblue"),
                    borderRadius: "1rem",
                    padding: "0rem 0.6rem",
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
            </>
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
    createPrompt(uuid, kind, question, config.sqlTemplate).then(e => {
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
      }}
    >
      <ModalHeader title="show prompt" handleClose={handleClose}></ModalHeader>
      <pre> {prompt} </pre>
    </Paper>
  );
}

function AllKindChat(props: { uuid: string; kind: DataSourceKind }) {
  const [config, setConfig] = useState<LLMConfig>(null);
  const { showNotification } = useContext(NotificationContext);
  useEffect(() => {
    const unlisten = appStore.onKeyChange(LLMConfig.key, () => {
      LLMConfig.get().then(config => {
        setConfig(config);
      });
    });
    return () => {
      unlisten.then(x => {
        x();
      });
    };
  }, []);

  useEffect(() => {
    LLMConfig.get().then(config => {
      if (!config) {
        showNotification("warning", "Please set LLM config first.");
        return;
      }
      setConfig(config);
    });
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
