import {
  appStore,
  ChatShowContext,
  DataSourceKind,
  extractLastNameFromUuid,
  handleCopyToClipboard,
  LLMConfig,
  LLMConfigContext,
  SaveBoolToLocalStorage,
  TabActiveContext,
  TableType,
  ViewKind,
} from "./helper.ts";
import CloseIcon from "@mui/icons-material/Close";
import {
  Autocomplete,
  Button,
  Checkbox,
  Divider,
  FormControlLabel,
  IconButton,
  Paper,
  Stack,
  Switch,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import { useCallback, useContext, useEffect, useRef, useState } from "react";
import { editor } from "monaco-editor";
import {
  AppMarkDown,
  SliderSize,
  SimpleFlexEditor,
  StackRowCenter,
  CancelButton,
} from "./CommonComp2.tsx";
import ArrowCircleUpIcon from "@mui/icons-material/ArrowCircleUp";
import EventNoteIcon from "@mui/icons-material/EventNote";
import EditIcon from "@mui/icons-material/Edit";
import { NotificationContext } from "./Notification.tsx";
import { PromptTemplate } from "@langchain/core/prompts";
import {
  AITextCompletionName,
  editorAppendContent,
  SetAutoCompletionName,
} from "./Common.ts";
import { ModalHeader } from "./CommonComp.tsx";
import { AllKindQuery } from "./Left.tsx";
import { invoke } from "@tauri-apps/api/core";
import types from "./ApiStruct.ts";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import { HumanMessage, AIMessage, BaseMessage } from "@langchain/core/messages";
import {
  callErrorable,
  createTableAndIndexSql,
  createViewSql,
  formatCreateTableAndIndexSql,
  getLocale,
  listCassandraAgg,
  listCreatedTypes,
  listEvent,
  listProcedure,
  listSequence,
  listTable,
  listUdf,
  listView,
  showCassandraAgg,
  showProcedure,
  showTableColumn,
  showUdf,
  tryGetMaxID,
  showViewColumns,
} from "./handlers.ts";

import * as mongoQueryBackend from "./MongoQueryBackend.ts";

class SimpleArrayMemory {
  history: BaseMessage[];
  k: number;
  constructor(k = 4) {
    this.history = [];
    this.k = k;
  }

  addUserMessage(message: string) {
    this.history.push(new HumanMessage(message));
    this._trimHistory(); // 添加后立即修剪
  }

  addAIMessage(message: string) {
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
  language?: string; // mongo used this as javascript or typescript
}) {
  const { uuid, kind, favoriteName, value, language } = props;
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const llmconfig = useContext(LLMConfigContext);
  const saveChatShow = useRef(new SaveBoolToLocalStorage("chat-show-" + uuid));
  const [show, setShow] = useState(saveChatShow.current.get());
  useEffect(() => {
    saveChatShow.current.save(show);
  }, [show]);
  const doQuery = useRef<() => void>(null);

  return (
    <Stack
      style={{
        height: "100%",
        width: "100%",
        position: "relative",
      }}
    >
      <ChatShowContext.Provider
        value={{
          show,
          setShow,
          doQuery,
        }}
      >
        <AllKindQuery
          uuid={uuid}
          kind={kind}
          handleEditor={e => {
            setEditor(e);
          }}
          favoriteName={favoriteName}
          value={value}
          language={language}
        />
        {llmconfig && editor && (
          <SqlChatTextInput uuid={uuid} kind={kind} editor={editor} />
        )}
      </ChatShowContext.Provider>
    </Stack>
  );
}

type DatabaseElements = {
  tables: types.ListTable[];
  views: types.ListTable[];
  materializedViews: types.ListTable[];
  udfs: types.NamedStaff[];
  types: types.CreatedTypeDef[];
  events: types.SqlEvent[];
  sequences: types.SequenceDetail[];
  cassandraAggs: types.NamedStaff[];
  procedures: types.NamedStaff[];
  uuid: string;
};

class MakePrompt {
  static getDefaultTemplatePredefineVariables(kind: DataSourceKind) {
    switch (kind) {
      case DataSourceKind.Redis:
        return MakePrompt.redisTemplate.inputVariables;
      case DataSourceKind.Mongo:
        return MakePrompt.mongoTemplate.inputVariables;
      default:
        return MakePrompt.sqlTemplate.inputVariables;
    }
  }

  static getDefaultTemplate(kind: DataSourceKind): string {
    switch (kind) {
      case DataSourceKind.Redis:
        return MakePrompt.redisTemplate.template as string;
      case DataSourceKind.Mongo:
        return MakePrompt.mongoTemplate.template as string;
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

  static mongoTemplate = PromptTemplate.fromTemplate(
    `system:You are a mongo database expert.
Your answer must follow ALL of these rules:
1. generate {language} only.
2. Don't generate markdown
3. Comments must use "//" syntax.
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
    public editor: editor.IStandaloneCodeEditor,
    public language?: string,
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
    const name = SetAutoCompletionName.get(this.editor.getModel().id);
    const typeHints = await invoke<object>("get_redis_type_hint", {
      name: this.uuid,
      text: this.question.trim(),
      queryId: name?.queryId,
    });

    let ret = "";
    for (const key in typeHints) {
      ret += `  ${key} has type '${typeHints[key]}'\n`;
    }
    return ret;
  }

  async createRedisPrompt(): Promise<string> {
    const inputs = {
      question: this.question.trim(),
      database_information: await this.redisDatabaseInformation(),
      chat_history: this.memory
        .getMessages()
        .map(x => x.toFormattedString())
        .join("\n"),
      locale: await getLocale(),
    };
    const userDefine = this.config.aiTextTemplates[this.kind];
    if (userDefine) {
      return PromptTemplate.fromTemplate(userDefine).format(inputs);
    }
    // default template
    return MakePrompt.redisTemplate.format(inputs);
  }

  mongoDatabaseInformation() {
    let ret = "we have some predefine some variable u can use.\n";
    ret += "```typescript";
    ret += mongoQueryBackend.interfaceRawText;
    ret += mongoQueryBackend.globalVar;
    ret += "```";
    return ret;
  }

  async createMongoPrompt(): Promise<string> {
    const inputs = {
      question: this.question.trim(),
      database_information: await this.mongoDatabaseInformation(),
      chat_history: this.memory
        .getMessages()
        .map(x => x.toFormattedString())
        .join("\n"),
      locale: await getLocale(),
      language: this.language,
    };
    const userDefine = this.config.aiTextTemplates[this.kind];
    if (userDefine) {
      return PromptTemplate.fromTemplate(userDefine).format(inputs);
    }
    // default template
    return MakePrompt.mongoTemplate.format(inputs);
  }

  async createSqlPrompt(): Promise<string> {
    const inputs = {
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
    const databases = await this.allSqlElements();
    for (const elements of databases) {
      ret += `==============================database ${extractLastNameFromUuid(
        elements.uuid,
      )}==============================\n`;

      for (const table of elements.tables) {
        if (!this.question.includes(table.name)) {
          continue;
        }
        const cols = await showTableColumn(elements.uuid, table.name);
        ret += `${table.name} is table
        Columns:${cols.map(x => x.field).join(",")}
        Primary key: ${cols
          .filter(x => x.is_primary)
          .map(x => (x.is_auto_increment ? `${x.field}(auto increment)` : x.field))
          .join(",")}\n`;
        {
          const maxID = await tryGetMaxID(elements.uuid, table.name);
          if (maxID) {
            ret += `${table.name} max id is ${maxID}\n`;
          }
        }
        {
          const def = await callErrorable(() => {
            return createTableAndIndexSql(elements.uuid, table.name);
          }, null);
          if (def) {
            ret += `${table.name} create table sql is:\n${formatCreateTableAndIndexSql(
              def,
            )}\n`;
          }
        }
      }
      for (const view of elements.views) {
        if (!this.question.includes(view.name)) {
          continue;
        }
        const cols = await showViewColumns(elements.uuid, view.name);
        ret += `${view.name} is view 
        Columns:${cols.map(x => x.field).join(",")}\n`;
        {
          const maxID = await tryGetMaxID(elements.uuid, view.name);
          if (maxID) {
            ret += `${view.name} max id is ${maxID}\n`;
          }
        }
        {
          const def = await callErrorable(() => {
            return createViewSql(elements.uuid, view.name, ViewKind.View);
          }, null);
          if (def) {
            ret += `${view.name} create view sql is:\n${def}\n`;
          }
        }
      }
      for (const view of elements.materializedViews) {
        if (!this.question.includes(view.name)) {
          continue;
        }
        const cols = await showViewColumns(elements.uuid, view.name);
        ret += `${view.name} is materalized view 
        Columns:${cols.map(x => x.field).join(",")}\n`;
        {
          const maxID = await tryGetMaxID(elements.uuid, view.name);
          if (maxID) {
            ret += `${view.name} max id is ${maxID}\n`;
          }
        }
        {
          const def = await callErrorable(() => {
            return createViewSql(elements.uuid, view.name, ViewKind.MaterializedView);
          }, null);
          if (def) {
            ret += `${view.name} create view sql is:\n${def}\n`;
          }
        }
      }

      // udf
      for (const udf of elements.udfs) {
        if (!this.question.includes(udf.name)) {
          continue;
        }
        ret += `${udf.name} is user define function \n`;
        const detail = await showUdf(elements.uuid, udf, this.kind);
        ret += `define sql is:\n${detail.def}\n`;
      }
      // cassandra agg
      for (const udf of elements.cassandraAggs) {
        if (!this.question.includes(udf.name)) {
          continue;
        }
        ret += `${udf.name} is user cassandra aggregate function \n`;
        const detail = await showCassandraAgg(
          elements.uuid,
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
        const detail = await showProcedure(elements.uuid, udf, this.kind);
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
    }

    return ret;
  }

  async allSqlElements(): Promise<DatabaseElements[]> {
    let fixed = [this.uuid];
    try {
      const name = SetAutoCompletionName.get(this.editor.getModel().id);
      fixed = await invoke<string[]>("ai_get_search_path", {
        name: this.uuid,
        queryId: name?.queryId,
        text: this.question,
      });
      console.log("ai search path", fixed);
    } catch (e) {
      console.log("get ai search path failed", e);
    }
    const ret: DatabaseElements[] = [];
    for (const uuid of fixed) {
      const tables = await callErrorable(async () => {
        return await listTable(uuid, TableType.Normal);
      });
      const views = await callErrorable(async () => {
        return await listView(uuid, ViewKind.View);
      });
      const materializedViews = await callErrorable(() => {
        return listView(uuid, ViewKind.MaterializedView);
      });
      const udfs = await callErrorable(async () => {
        return await listUdf(uuid);
      });

      const events = await callErrorable(async () => {
        return await listEvent(uuid);
      });
      const sequences = await callErrorable(async () => {
        return await listSequence(uuid);
      });

      const procedures = await callErrorable(async () => {
        return await listProcedure(uuid);
      });

      const types = await callErrorable(async () => {
        return await listCreatedTypes(uuid);
      });
      const cassandraAggs =
        this.kind == DataSourceKind.Cassandra
          ? await callErrorable(async () => {
              return await listCassandraAgg(uuid);
            })
          : [];
      ret.push({
        tables,
        views,
        materializedViews,
        udfs,
        events,
        sequences,
        types,
        procedures,
        cassandraAggs,
        uuid,
      });
    }
    return ret;
  }
  createPrompt(): Promise<string> {
    switch (this.kind) {
      case DataSourceKind.Mongo:
        return this.createMongoPrompt();
      case DataSourceKind.Redis:
        return this.createRedisPrompt();
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
  const [prevQuestion, setPrevQuestion] = useState("");
  const [newQuestion, setNewQuestion] = useState("");
  const { showNotification } = useContext(NotificationContext);
  const [showPrompt, setShowPrompt] = useState(false);
  const [chatHistory, setChatHistory] = useState<string[]>([]);
  const [showChatHistory, setShowChatHistory] = useState(false);
  const chatMemory = useRef(new SimpleArrayMemory());
  const [showEditPromptTemplate, setShowEditPromptTemplate] = useState(false);
  const [invokeStatus, setInvokeStatus] = useState("");
  const chatMemoryK = useRef<number>(4);
  const [cancelFN, setCancelFN] = useState<() => void>(null);
  const saveCleanEditor = useRef(new SaveBoolToLocalStorage("ai-chat-clean-editor"));
  const { show, setShow, doQuery } = useContext(ChatShowContext);
  const [cleanEditor, setCleanEditor] = useState(saveCleanEditor.current.get());
  const saveQueryAfter = useRef(new SaveBoolToLocalStorage("ai-chat-query-after"));
  const [queryAfter, setQueryAfter] = useState(saveQueryAfter.current.get());

  const vis = useContext(TabActiveContext);
  useEffect(() => {
    const closeThis = (e: KeyboardEvent) => {
      if (e.key === "Escape" && vis) {
        e.preventDefault();
        setShow(false);
      }
    };
    document.addEventListener("keydown", closeThis);
    return () => {
      document.removeEventListener("keydown", closeThis);
    };
  }, [vis, setShow]);

  useEffect(() => {
    saveCleanEditor.current.save(cleanEditor);
  }, [cleanEditor]);

  useEffect(() => {
    saveQueryAfter.current.save(queryAfter);
  }, [queryAfter]);

  const makePrompt = async () => {
    chatMemory.current.setK(chatMemoryK.current);
    try {
      const prompt = await new MakePrompt(
        uuid,
        config,
        kind,
        newQuestion,
        chatMemory.current,
        editor,
        editor.getModel().getLanguageId(),
      ).createPrompt();
      return prompt;
    } catch (e) {
      showNotification("error", "make prompt failed", e);
      return null;
    }
  };

  const handleChat = async () => {
    try {
      // 清空内容，同时保留在 Undo 栈里
      if (cleanEditor) {
        editor.executeEdits("my-source", [
          {
            range: editor.getModel().getFullModelRange(), // 全选
            text: "",
          },
        ]);
      } else {
        editorAppendContent(editor, "\n");
      }
      setPrevQuestion(newQuestion);
      setInvokeStatus("making prompt");
      chatMemory.current.addUserMessage(newQuestion);
      setChatHistory(e => [...e, newQuestion]);
      let wholeText = "";
      setInvokeStatus("trying getting stream response");
      const prompt = await makePrompt();
      if (!prompt) {
        return;
      }
      let i = 0;
      await config.stream(
        prompt,
        msg => {
          setInvokeStatus("receiving stream response#" + i);
          editorAppendContent(editor, msg);
          wholeText += msg;
          i++;
        },
        f => {
          setCancelFN(() => {
            // 这里必须写一个callback不然会被直接执行
            return f;
          });
        },
      );
      setInvokeStatus("done");
      setTimeout(() => {
        setInvokeStatus("");
      }, 5000);
      setChatHistory(e => [...e, wholeText]);
      chatMemory.current.addAIMessage(wholeText);
      if (queryAfter) {
        if (!doQuery.current) {
          showNotification("error", "do query is null ,this is a internal error");
        } else {
          doQuery.current();
        }
      }
    } catch (e) {
      showNotification("error", "call llm failed", e);
    } finally {
      setCancelFN(null);
    }
  };

  return (
    <>
      <Paper
        style={{
          height: "30vh",
          width: "80%",
          display: show ? "flex" : "none",
          position: "absolute",
          flexDirection: "column",
          bottom: 0,
          left: "10%",
          border: "2px solid gray",
          borderRadius: "1rem",
          zIndex: 14,
          opacity: 0.9,
        }}
      >
        <Stack
          direction="row"
          style={{ alignItems: "center", justifyContent: "space-between" }}
        >
          <Stack
            direction="row"
            style={{ alignItems: "center", overflow: "auto", paddingBottom: "0.5rem" }}
          >
            {cancelFN ? (
              <CancelButton
                handleCancel={() => {
                  cancelFN();
                  setCancelFN(null);
                }}
              />
            ) : (
              <IconButton onClick={handleChat} disabled={newQuestion == prevQuestion}>
                <ArrowCircleUpIcon
                  style={{ fontSize: "1.8rem" }}
                  color={newQuestion == prevQuestion ? "disabled" : "primary"}
                />
              </IconButton>
            )}
            <Tooltip title="history">
              <IconButton
                onClick={() => {
                  setShowChatHistory(e => !e);
                }}
                disabled={chatHistory.length == 0}
              >
                <EventNoteIcon
                  color={chatHistory.length == 0 ? "disabled" : "primary"}
                />
              </IconButton>
            </Tooltip>
            <Tooltip title="show prompt">
              <Button
                disabled={newQuestion.trim() == ""}
                onClick={() => {
                  setShowPrompt(e => !e);
                }}
                variant={showPrompt ? "contained" : "text"}
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
                <EditIcon color="secondary" />
              </IconButton>
            </Tooltip>
            <SliderSize
              storeKey="chat-memory-buffer-size"
              max={20}
              min={0}
              step={2}
              handleValue={value => {
                chatMemoryK.current = value;
              }}
              notice="chat memory buffer size"
            />
            <Tooltip title="delete all content from the editor before we receive the response">
              <FormControlLabel
                control={
                  <Checkbox
                    checked={cleanEditor}
                    onChange={e => setCleanEditor(e.target.checked)}
                  />
                }
                label="clean editor"
              />
            </Tooltip>
            <Tooltip title="do query after we receive the response">
              <FormControlLabel
                control={
                  <Switch
                    checked={queryAfter}
                    onChange={e => setQueryAfter(e.target.checked)}
                  />
                }
                label="auto query"
              />
            </Tooltip>
          </Stack>

          <Stack direction="row">
            <IconButton
              onClick={() => {
                setShow(false);
              }}
            >
              <CloseIcon />
            </IconButton>
          </Stack>
        </Stack>

        <Divider> </Divider>
        <SimpleFlexEditor
          language="text"
          handleValue={v => {
            setNewQuestion(v);
          }}
          barLeft={invokeStatus && <Typography color="info">{invokeStatus}</Typography>}
          handleEditor={e => {
            AITextCompletionName.set(e.getModel().id, editor.getModel().id);
          }}
          dontFocus
        />
      </Paper>
      {showChatHistory && (
        <ChatHistory
          history={chatHistory}
          handleClose={() => {
            setShowChatHistory(false);
          }}
          handleClean={() => {
            setChatHistory([]);
          }}
        />
      )}
      {showEditPromptTemplate && (
        <EditPromptTemplate
          config={config}
          handleClose={() => {
            setShowEditPromptTemplate(false);
          }}
          kind={kind}
        />
      )}
      {showPrompt && (
        <ShowPrompt
          handleClose={() => {
            setShowPrompt(false);
          }}
          makePrompt={makePrompt}
        />
      )}
    </>
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
  const [templateVar, setTemplateVar] = useState("");
  const handleSave = async () => {
    try {
      const config = await LLMConfig.get();
      if (!config.aiTextTemplates) {
        config.aiTextTemplates = {};
      }
      config.aiTextTemplates[kind] = newTemplate;
      await config.save();
      showNotification("success", "save template ok");
      handleClose();
    } catch (e) {
      showNotification("error", "save template failed", e);
      return;
    }
  };
  const handleDelete = async () => {
    try {
      const config = await LLMConfig.get();
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
        e.preventDefault();
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
        <ModalHeader title="edit template" handleClose={handleClose} />
        <SimpleFlexEditor
          value={newTemplate}
          handleValue={v => {
            setNewTemplate(v);
          }}
          handleEditor={e => {
            setEditor(e);
          }}
          language="text"
        />
        <Divider />
        <StackRowCenter>
          <Button onClick={handleSave}> save </Button>
          <Autocomplete
            options={MakePrompt.getDefaultTemplatePredefineVariables(kind)}
            style={{ width: "22rem" }}
            value={templateVar}
            renderInput={params => (
              <TextField {...params} label="variable can be use in template" />
            )}
            freeSolo={false}
            onChange={(_, templateVar) => {
              editor
                .getModel()
                .pushEditOperations(
                  null,
                  [{ range: editor.getSelection(), text: `{${templateVar}}` }],
                  null,
                );
              editor.focus();
              setTemplateVar(templateVar);
            }}
          />

          <Tooltip title="delete template,after we will use system default template">
            <IconButton onClick={handleDelete}>
              <DeleteForeverIcon> </DeleteForeverIcon>
            </IconButton>
          </Tooltip>
        </StackRowCenter>
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
      />
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
                  <AppMarkDown>{his}</AppMarkDown>
                </div>
              </Stack>
              {index > 0 && (index + 1) % 2 == 0 && <Divider />}
            </div>
          );
        })}
      </Stack>
    </Paper>
  );
}

function ShowPrompt(props: {
  makePrompt: () => Promise<string>;
  handleClose: () => void;
}) {
  const { handleClose, makePrompt } = props;
  const [prompt, setPrompt] = useState("");
  const { showNotification } = useContext(NotificationContext);
  useEffect(() => {
    makePrompt().then(e => {
      setPrompt(e);
    });
  }, [makePrompt]);
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
        />
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
  language?: string;
}) {
  const [config, setConfig] = useState<LLMConfig>(null);
  const { showNotification } = useContext(NotificationContext);

  const handleReload = useCallback(() => {
    LLMConfig.get()
      .then(config => {
        if (config && config.enable()) {
          setConfig(config);
        }
      })
      .catch(e => {
        showNotification("error", "get llm config failed", e);
      });
  }, [showNotification]);
  useEffect(() => {
    const unlisten = appStore.onKeyChange(LLMConfig.key, () => {
      handleReload();
    });
    return () => {
      unlisten.then(x => {
        x();
      });
    };
  }, [handleReload]);

  useEffect(() => {
    handleReload();
  }, [handleReload]);

  return (
    <LLMConfigContext.Provider value={config}>
      <Chat {...props} />
    </LLMConfigContext.Provider>
  );
}

export { AllKindChat };
