import {
  ChatShowContext,
  DataSourceKind,
  extractLastNameFromUuid,
  handleCopyToClipboard,
  TabActiveContext,
  TableType,
  ViewKind,
} from "./helper.ts";
import CloseIcon from "@mui/icons-material/Close";
import {
  Autocomplete,
  Box,
  Button,
  Checkbox,
  Divider,
  FormControlLabel,
  IconButton,
  Modal,
  Paper,
  Stack,
  Switch,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import { useContext, useEffect, useRef, useState } from "react";
import { editor } from "monaco-editor";
import ClearIcon from "@mui/icons-material/Clear";
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
import { PromptTemplate } from "@langchain/core/prompts";
import {
  AITextCompletionName,
  convertPathToDataUrl,
  editorAppendContent,
  EscDownEvent,
  modalHandleClose,
  modalStyle,
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
import {
  useAIPrompt,
  useBoolSwitch,
  useLLM,
  useNotification,
  UseSizeConfig,
  useSizeStore,
} from "./store.tsx";

import AddPhotoAlternateIcon from "@mui/icons-material/AddPhotoAlternate";
import { open } from "@tauri-apps/plugin-dialog";
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

const showChatKey = "chat-show";

function Chat(props: {
  uuid: string;
  kind: DataSourceKind;
  value?: string;
  favoriteName?: string;
  language?: string; // mongo used this as javascript or typescript
}) {
  const { uuid, kind, favoriteName, value, language } = props;
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const llm = useLLM();

  const saveChatShow = useBoolSwitch();
  const [show, setShow] = useState(saveChatShow.switches[showChatKey]);
  const doQuery = useRef<() => void>(null);

  return (
    <Stack
      style={{
        height: "100%",
        width: "100%",
        position: "relative",
      }}>
      <ChatShowContext.Provider
        value={{
          show,
          setShow,
          doQuery,
        }}>
        <AllKindQuery
          uuid={uuid}
          kind={kind}
          handleEditor={(e) => {
            setEditor(e);
          }}
          favoriteName={favoriteName}
          value={value}
          language={language}
        />
        {llm && editor && <SqlChatTextInput uuid={uuid} kind={kind} editor={editor} />}
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

  async createRedisPrompt(userDefine: string): Promise<string> {
    const inputs = {
      question: this.question.trim(),
      database_information: await this.redisDatabaseInformation(),
      chat_history: this.memory
        .getMessages()
        .map((x) => x.toFormattedString())
        .join("\n"),
      locale: await getLocale(),
    };
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

  async createMongoPrompt(userDefine): Promise<string> {
    const inputs = {
      question: this.question.trim(),
      database_information: await this.mongoDatabaseInformation(),
      chat_history: this.memory
        .getMessages()
        .map((x) => x.toFormattedString())
        .join("\n"),
      locale: await getLocale(),
      language: this.language,
    };
    if (userDefine) {
      return PromptTemplate.fromTemplate(userDefine).format(inputs);
    }
    // default template
    return MakePrompt.mongoTemplate.format(inputs);
  }

  async createSqlPrompt(userDefine): Promise<string> {
    const inputs = {
      dialect: this.kind2CommonName(),
      question: this.question,
      database_information: await this.sqlDatabaseInformation(),
      chat_history:
        this.memory
          .getMessages()
          .map((x) => x.toFormattedString())
          .join("\n") + "\n",
      locale: await getLocale(),
    };
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

      if (elements.tables.length > 0) {
        ret += `we have fellow tables: ${elements.tables.map((x) => x.name).join(",")}\n`;
      }
      if (elements.views.length > 0) {
        ret += `we have fellow view: ${elements.views.map((x) => x.name).join(",")}\n`;
      }
      if (elements.materializedViews.length > 0) {
        ret += `we have fellow materialized view: ${elements.materializedViews.map((x) => x.name).join(",")}\n`;
      }
      if (elements.udfs.length > 0) {
        ret += `we have fellow user define function: ${elements.udfs.map((x) => x.name).join(",")}\n`;
      }
      if (elements.types.length > 0) {
        ret += `we have fellow user define type: ${elements.types.map((x) => x.name).join(",")}\n`;
      }
      if (elements.events.length > 0) {
        ret += `we have fellow user define event:  ${elements.events.map((x) => x.name).join(",")}\n`;
      }
      if (elements.sequences.length > 0) {
        ret += `we have fellow user define sequence: ${elements.sequences.map((x) => x.name).join(",")}\n`;
      }
      if (elements.cassandraAggs.length > 0) {
        ret += `we have fellow user define aggregate function: ${elements.cassandraAggs.map((x) => x.name).join(",")}\n`;
      }
      if (elements.procedures.length > 0) {
        ret += `we have fellow user define procedure:  ${elements.procedures.map((x) => x.name).join(",")}\n`;
      }

      for (const table of elements.tables) {
        if (!this.question.includes(table.name)) {
          continue;
        }
        const cols = await showTableColumn(elements.uuid, table.name);
        ret += `${table.name} is table
        Columns:${cols.map((x) => x.field).join(",")}
        Primary key: ${cols
          .filter((x) => x.is_primary)
          .map((x) => (x.is_auto_increment ? `${x.field}(auto increment)` : x.field))
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
        Columns:${cols.map((x) => x.field).join(",")}\n`;
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
        Columns:${cols.map((x) => x.field).join(",")}\n`;
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
  createPrompt(userDefine: string | null): Promise<string> {
    switch (this.kind) {
      case DataSourceKind.Mongo:
        return this.createMongoPrompt(userDefine);
      case DataSourceKind.Redis:
        return this.createRedisPrompt(userDefine);
      default:
        return this.createSqlPrompt(userDefine);
    }
  }
}
const queryAfterKey = "query-after-switch";
const cleanEditorKey = "clean-editor-key";

function SqlChatTextInput(props: {
  uuid: string;
  kind: DataSourceKind;
  editor: editor.IStandaloneCodeEditor;
}) {
  const { uuid, kind, editor } = props;
  const llm = useLLM();
  const [question, setQuestion] = useState("");
  const { showNotification } = useNotification();
  const [showPrompt, setShowPrompt] = useState(false);
  const [chatHistory, setChatHistory] = useState<string[]>([]);
  const [showChatHistory, setShowChatHistory] = useState(false);
  const chatMemory = useRef(new SimpleArrayMemory());
  const [showEditPromptTemplate, setShowEditPromptTemplate] = useState(false);
  const [invokeStatus, setInvokeStatus] = useState("");
  const [cancelFN, setCancelFN] = useState<() => void>(null);
  const { show, setShow, doQuery } = useContext(ChatShowContext);
  const getSize = useSizeStore((s) => s.getSize);
  const { switches, setSwitch } = useBoolSwitch();
  const queryAfter = switches[queryAfterKey];
  const cleanEditor = switches[cleanEditorKey];
  const [images, setImages] = useState<string[]>([]);

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
    if (!show) {
      setShowEditPromptTemplate(false);
      setShowChatHistory(false);
      setShowPrompt(false);
    }
  }, [show]);

  const getPrompt = useAIPrompt((s) => s.getPrompt);
  const userDefine = getPrompt(kind);

  const makePrompt = async () => {
    chatMemory.current.setK(getSize(UseSizeConfig.chatMemoryKey));
    try {
      const prompt = await new MakePrompt(
        uuid,
        kind,
        question,
        chatMemory.current,
        editor,
        editor.getModel().getLanguageId(),
      ).createPrompt(userDefine);
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
      setInvokeStatus("making prompt");
      chatMemory.current.addUserMessage(question);
      setChatHistory((e) => [...e, question]);
      let wholeText = "";
      setInvokeStatus("trying getting stream response");
      const prompt = await makePrompt();
      if (!prompt) {
        return;
      }
      let i = 0;
      await llm.stream(
        prompt,
        images,
        (msg) => {
          setInvokeStatus("receiving stream response#" + i);
          editorAppendContent(editor, msg);
          wholeText += msg;
          i++;
        },
        (f) => {
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
      setChatHistory((e) => [...e, wholeText]);
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
          height: "40vh",
          width: "80%",
          display: show ? "flex" : "none",
          position: "absolute",
          flexDirection: "column",
          bottom: 0,
          left: "10%",
          border: "2px solid gray",
          borderRadius: "1rem",
          zIndex: 14,
        }}>
        <Stack
          direction="row"
          style={{ alignItems: "center", justifyContent: "space-between" }}>
          <Stack
            direction="row"
            style={{ alignItems: "center", overflow: "auto", paddingBottom: "0.5rem" }}>
            {cancelFN ? (
              <CancelButton
                handleCancel={() => {
                  cancelFN();
                  setCancelFN(null);
                }}
              />
            ) : (
              <IconButton onClick={handleChat}>
                <ArrowCircleUpIcon style={{ fontSize: "1.8rem" }} />
              </IconButton>
            )}
            <Tooltip title="history">
              <IconButton
                onClick={() => {
                  setShowChatHistory((e) => !e);
                }}
                disabled={chatHistory.length == 0}>
                <EventNoteIcon
                  color={chatHistory.length == 0 ? "disabled" : "primary"}
                />
              </IconButton>
            </Tooltip>
            <Tooltip title="show prompt">
              <Button
                disabled={question.trim() == ""}
                onClick={() => {
                  setShowPrompt((e) => !e);
                }}
                variant={showPrompt ? "contained" : "text"}>
                prompt
              </Button>
            </Tooltip>
            <Tooltip title="edit prompt template">
              <IconButton
                onClick={() => {
                  setShowEditPromptTemplate((e) => !e);
                }}>
                <EditIcon color="secondary" />
              </IconButton>
            </Tooltip>
            <SliderSize config={UseSizeConfig.chatMemoryKey} />
            <Tooltip title="delete all content from the editor before we receive the response">
              <FormControlLabel
                control={
                  <Checkbox
                    checked={cleanEditor}
                    onChange={(e) => setSwitch(cleanEditorKey, e.target.checked)}
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
                    onChange={(e) => setSwitch(queryAfterKey, e.target.checked)}
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
              }}>
              <CloseIcon />
            </IconButton>
          </Stack>
        </Stack>

        <Stack direction="row">
          <IconButton
            onClick={async () => {
              let files = await open({ multiple: true, pickerMode: "image" });
              if (!files) {
                return;
              }
              if (typeof files == "string") {
                files = [files];
              }
              try {
                for (const file of files) {
                  const fileContent = await convertPathToDataUrl(file);
                  setImages([...images, fileContent]);
                }
              } catch (e) {
                showNotification("error", "open image failed", e);
              }
            }}>
            <AddPhotoAlternateIcon />
          </IconButton>
          <ShowChatImages images={images} setImages={setImages} />
        </Stack>

        <Divider> </Divider>
        <SimpleFlexEditor
          language="text"
          barLeft={invokeStatus && <Typography color="info">{invokeStatus}</Typography>}
          handleEditor={(e) => {
            AITextCompletionName.set(e.getModel().id, editor.getModel().id);
          }}
          handleValue={(v) => {
            setQuestion(v);
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

function ShowChatImages(props: { images: string[]; setImages }) {
  const { images, setImages } = props;
  const [msgDetail, setMsgDetail] = useState("");

  const maxSize = "12rem";
  return (
    <Stack direction="row">
      {images.map((e, index) => {
        return (
          <img
            key={index}
            src={e}
            style={{ maxWidth: maxSize, maxHeight: maxSize, cursor: "pointer" }}
            onClick={() => {
              setMsgDetail(e);
            }}
          />
        );
      })}
      {msgDetail && (
        <Modal
          open={!!msgDetail}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setMsgDetail("");
          })}>
          <Box sx={{ ...modalStyle }} width="80vw" height="60vh">
            <Stack direction="row">
              <IconButton
                onClick={() => {
                  setImages(images.filter((e) => e != msgDetail));
                  setMsgDetail("");
                }}>
                <ClearIcon />
              </IconButton>
            </Stack>

            <img src={msgDetail} style={{ width: "100%" }} />
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function EditPromptTemplate(props: { kind: DataSourceKind; handleClose: () => void }) {
  const { kind, handleClose } = props;
  const { getPrompt, setPrompt, removePrompt } = useAIPrompt();
  const userDefine = getPrompt(kind);

  const [newTemplate, setNewTemplate] = useState(
    userDefine ? userDefine : MakePrompt.getDefaultTemplate(kind),
  );
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const { showNotification } = useNotification();
  const [templateVar, setTemplateVar] = useState("");
  const handleSave = async () => {
    setPrompt(kind, newTemplate);
  };
  const handleDelete = async () => {
    removePrompt(kind);
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
      }}>
      <Stack style={{ height: "100%" }}>
        <ModalHeader title="edit template" handleClose={handleClose} />
        <SimpleFlexEditor
          value={newTemplate}
          handleValue={(v) => {
            setNewTemplate(v);
          }}
          handleEditor={(e) => {
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
            renderInput={(params) => (
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
      ref={wrapperRef}>
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
                }}>
                <div
                  style={{
                    width: "85%",
                  }}>
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
  const { showNotification } = useNotification();
  useEffect(() => {
    makePrompt().then((e) => {
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
      }}>
      <Stack height="100%">
        <ModalHeader
          headerExtra=<>
            <Button
              variant="text"
              onClick={() => {
                handleCopyToClipboard(prompt, showNotification);
              }}>
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

export { Chat as AllKindChat };
