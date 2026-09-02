import {
  ChatShowContext,
  DataSourceKind,
  extractConfigNameFromUuid,
  QueryIDEditorChatCtx,
  secondConfirm,
  DbnameContext,
} from "./helper.ts";
import { CloseIcon, SaveIcon } from "./icons";
import { TbPrompt, UploadFileIcon } from "./icons";
import {
  Chip,
  CircularProgress,
  Dialog,
  Divider,
  IconButton,
  Pagination,
  Paper,
  Stack,
  Tooltip,
  Typography,
} from "@mui/material";
import {
  Dispatch,
  ReactNode,
  SetStateAction,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { editor } from "monaco-editor";
import {
  SliderSize,
  SimpleFlexEditor,
  CancelButton,
  EscPressedDo,
  ClearableTextField,
  AppMarkDown,
} from "./CommonComp2.tsx";
import { ArrowCircleUpIcon, EventNoteIcon } from "./icons";
import {
  convertPathToDataUrl,
  editorAppendContent,
  EscDownEvent,
  extractBaseName,
  modalHandleClose,
  providerQueryID,
  uuidv4,
} from "./Common.ts";
import { ModalHeader } from "./CommonComp.tsx";
import { DeleteForeverIcon, NavigateNextIcon, NavigateBeforeIcon } from "./icons";

import {
  useBoolSwitch,
  useChatStore,
  useLLM,
  useNotification,
  useUserSystemPromptStore,
  UseSizeConfig,
  useSizeStore,
} from "./store.tsx";

import { AddPhotoAlternateIcon } from "./icons";
import { open } from "@tauri-apps/plugin-dialog";

import { useDebounce } from "use-debounce";
import { SqlQuery } from "./SqlQuery.tsx";
import { RedisQuery } from "./RedisQuery.tsx";
import { MongoQuery } from "./MongoQuery.tsx";
import { CouchBaseQuery } from "./CouchBase.tsx";
import { MdCleaningServices } from "./icons";
import { openPath } from "@tauri-apps/plugin-opener";

function AllKindQuery(props: { value?: string; favoriteName?: string }) {
  const { value, favoriteName } = props;
  const { kind } = useContext(DbnameContext);
  let content: ReactNode;
  switch (kind) {
    case DataSourceKind.Redis:
      content = <RedisQuery value={value} favoriteName={favoriteName} />;
      break;
    case DataSourceKind.Mongo:
      content = <MongoQuery value={value} favoriteName={favoriteName} />;
      break;
    case DataSourceKind.CouchBase:
      return <CouchBaseQuery value={value} />;
    default:
      content = <SqlQuery value={value} favoriteName={favoriteName} />;
      break;
  }
  return <div style={{ height: "100%" }}> {content}</div>;
}

function AllKindChat(props: { value?: string; favoriteName?: string }) {
  const { dbName } = useContext(DbnameContext);
  const { favoriteName, value } = props;
  const llm = useLLM();
  const showChatKey = "chat-show-" + dbName;
  const { getSwitch, setSwitch } = useBoolSwitch();
  const [show, setShow] = useState(getSwitch(showChatKey));

  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  useEffect(() => {
    setSwitch(showChatKey, show);
  }, [setSwitch, show, showChatKey]);

  let queryId = useRef(providerQueryID.id ? providerQueryID.id : uuidv4());
  return (
    <QueryIDEditorChatCtx.Provider
      value={{ queryId: queryId.current, editor, setEditor }}>
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
          }}>
          <AllKindQuery favoriteName={favoriteName} value={value} />
          {llm && editor && <SqlChatTextInput />}
        </ChatShowContext.Provider>
      </Stack>
    </QueryIDEditorChatCtx.Provider>
  );
}

const cleanEditorKey = "clean-editor-key";

function SqlChatTextInput() {
  const { dbName } = useContext(DbnameContext);
  const { editor } = useContext(QueryIDEditorChatCtx);
  const llm = useLLM();
  const [question, setQuestion] = useState("");
  const { showNotification } = useNotification();
  const [showChatHistory, setShowChatHistory] = useState(false);
  const [showEditPrompt, setShowEditPrompt] = useState(false);
  const [invokeStatus, setInvokeStatus] = useState("");
  const [cancelFN, setCancelFN] = useState<() => void>(null);
  const { show, setShow } = useContext(ChatShowContext);
  const { getSwitch, setSwitch } = useBoolSwitch();
  const [images, setImages] = useState<string[]>([]);
  const [documents, setDocuments] = useState<string[]>([]);
  const { getSize } = useSizeStore();
  const { messagesByNames, saveMessage } = useChatStore();
  const connName = extractConfigNameFromUuid(dbName);
  const { prompts: userDefinePrompts } = useUserSystemPromptStore();
  const cleanEditor = getSwitch(cleanEditorKey);
  useEffect(() => {
    if (!show) {
      setShowChatHistory(false);
      setShowEditPrompt(false);
    }
  }, [show]);

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
      let wholeText = "";
      setInvokeStatus("trying getting stream response");
      let i = 0;
      let queryId = editor.getModel()._dbpal_query_id;
      saveMessage(connName, "user", question);
      // messagesByNames is because of `ChatHistory` will render and fetch ,and this tricky.
      const history = messagesByNames[connName];
      await llm.stream_prompt(
        queryId.name,
        queryId.queryId,
        question,
        images,
        documents,
        (function () {
          if (!history || history.length === 0) {
            return [];
          }
          // Use a negative index in slice to get the last N elements
          const size = getSize(UseSizeConfig.chatMemoryKey);
          return history
            .slice(-size) // This takes the last 'size' elements
            .map((h) => {
              return {
                role: h.role,
                msg: h.content,
              };
            });
        })(),
        userDefinePrompts[connName],
        (event) => {
          setInvokeStatus("receiving stream response#" + i);

          switch (event.resp_type) {
            case "content":
              editorAppendContent(editor, event.msg);
              wholeText += event.msg;
              i++;
              break;
            case "tokenusage":
              showNotification({
                severity: "info",
                title: "token usage",
                message: `input token:${event.input_token}  output token:${event.output_token} total token:${event.total_token}`,
              });
              break;
          }
        },
        (f) => {
          setCancelFN(() => {
            // 这里必须写一个callback不然会被直接执行
            return f;
          });
        },
      );
      showNotification({ severity: "success", title: "call llm success" });
      setInvokeStatus("done");
      setTimeout(() => {
        setInvokeStatus("");
      }, 5000);
      saveMessage(connName, "assistant", wholeText);
    } catch (e) {
      showNotification({ severity: "error", title: "call llm failed", message: e });
    } finally {
      setCancelFN(null);
    }
  };

  return (
    <>
      <EscPressedDo
        doingIt={() => {
          return show;
        }}
        setUndo={() => {
          setShow(false);
        }}
      />
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
          overflow: "hidden",
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
              <IconButton onClick={handleChat} disabled={!question}>
                <ArrowCircleUpIcon style={{ fontSize: "1.8rem" }} />
              </IconButton>
            )}
            <Tooltip title="history">
              <IconButton
                onClick={() => {
                  setShowChatHistory((e) => !e);
                }}>
                <EventNoteIcon />
              </IconButton>
            </Tooltip>

            <Tooltip title="edit user prompt,this send to LLM every time">
              <IconButton
                onClick={() => {
                  setShowEditPrompt((e) => !e);
                }}>
                <TbPrompt style={{ fontSize: "1.8rem" }} />
              </IconButton>
            </Tooltip>
            <SliderSize config={UseSizeConfig.chatMemoryKey} />
            <Tooltip title="delete all content from the editor before we receive the response">
              <IconButton onClick={() => setSwitch(cleanEditorKey, !cleanEditor)}>
                <MdCleaningServices
                  color={cleanEditor ? "rgb(42, 59, 240)" : ""}
                  fontSize="1.2rem"
                />
              </IconButton>
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

        <Stack direction="row" style={{ alignItems: "flex-end" }}>
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
                  if (images.find((x) => x == fileContent)) {
                    showNotification({
                      severity: "error",
                      title: "image have already been add",
                    });
                  } else {
                    setImages((e) => [...e, fileContent]);
                  }
                }
              } catch (e) {
                showNotification({
                  severity: "error",
                  title: "open image failed",
                  message: e,
                });
              }
            }}>
            <AddPhotoAlternateIcon />
          </IconButton>

          {images.length > 0 && (
            <IconButton
              color="error"
              onClick={async () => {
                if (
                  !(await secondConfirm({
                    confirmText: "remove",
                    title: "remove all images ",
                  }))
                ) {
                  return;
                }
                setImages([]);
              }}>
              <DeleteForeverIcon />
            </IconButton>
          )}
          <ShowChatImages images={images} setImages={setImages} />
        </Stack>

        <Stack direction="row" style={{ alignItems: "flex-end" }}>
          <IconButton
            onClick={async () => {
              let files = await open({ multiple: true, pickerMode: "document" });
              if (!files) {
                return;
              }
              if (typeof files == "string") {
                files = [files];
              }
              try {
                for (const file of files) {
                  if (documents.find((e) => e == file)) {
                    showNotification({
                      severity: "error",
                      title: "image have already been add",
                    });
                  } else {
                    setDocuments((x) => [...x, file]);
                  }
                }
              } catch (e) {
                showNotification({
                  severity: "error",
                  title: "open image failed",
                  message: e,
                });
              }
            }}>
            <UploadFileIcon />
          </IconButton>
          {documents.length > 0 && (
            <IconButton
              color="error"
              onClick={async () => {
                if (
                  !(await secondConfirm({
                    confirmText: "remove",
                    title: "remove all documents",
                  }))
                ) {
                  return;
                }
                setDocuments([]);
              }}>
              <DeleteForeverIcon />
            </IconButton>
          )}

          <ShowChatDocuments documents={documents} setDocuments={setDocuments} />
        </Stack>
        <Divider> </Divider>
        <SimpleFlexEditor
          language="text"
          barLeft={invokeStatus && <Typography color="info">{invokeStatus}</Typography>}
          handleValue={(v) => {
            setQuestion(v);
          }}
          handleEditor={(e) => {
            e.getModel()._dbpal_query_id = {
              ...editor.getModel()._dbpal_query_id,
            };
          }}
        />
      </Paper>

      <ChatHistory
        connName={connName}
        handleClose={() => {
          setShowChatHistory(false);
        }}
        showChatHistory={showChatHistory}
      />

      <EditPrompt
        connName={connName}
        showEdit={showEditPrompt}
        handleClose={() => {
          setShowEditPrompt(false);
        }}
      />
    </>
  );
}

function ShowChatDocuments(props: {
  documents: string[];
  setDocuments: Dispatch<SetStateAction<string[]>>;
}) {
  const { documents, setDocuments } = props;
  return (
    <Stack direction="row" style={{ alignItems: "center" }}>
      {documents.map((path, index) => (
        <Chip
          key={index}
          label={extractBaseName(path)}
          style={{ cursor: "pointer" }}
          onClick={() => {
            openPath(path);
          }}
          onDelete={() => {
            setDocuments((x) => x.filter((e) => e != path));
          }}
        />
      ))}
    </Stack>
  );
}
function ShowChatImages(props: { images: string[]; setImages }) {
  const { images, setImages } = props;
  const [showMsgIndex, setMsgShowIndex] = useState<number | null>(null);
  const maxSize = "4vw";
  const show = typeof showMsgIndex == "number";
  return (
    <Stack direction="row">
      {images.map((e, index) => {
        return (
          <img
            key={index}
            src={e}
            style={{ maxWidth: maxSize, maxHeight: maxSize, cursor: "pointer" }}
            onClick={() => {
              setMsgShowIndex(index);
            }}
          />
        );
      })}
      {show && (
        <Dialog
          fullWidth
          maxWidth="md"
          open={show}
          slotProps={{
            paper: {
              style: {
                height: "55vh",
                overflow: "auto",
              },
            },
          }}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setMsgShowIndex(null);
          })}>
          <Stack direction="row" style={{ alignItems: "center" }}>
            #{showMsgIndex + 1}
            <IconButton
              onClick={() => {
                setImages(images.filter((_e, idx) => idx != showMsgIndex));
                setMsgShowIndex(null);
              }}
              color="warning">
              <DeleteForeverIcon />
            </IconButton>
            <IconButton>
              <NavigateBeforeIcon
                onClick={() => {
                  if (showMsgIndex == 0) {
                    setMsgShowIndex(images.length - 1);
                  } else {
                    setMsgShowIndex(showMsgIndex - 1);
                  }
                }}
              />
            </IconButton>
            <IconButton>
              <NavigateNextIcon
                onClick={() => {
                  if (showMsgIndex == images.length - 1) {
                    setMsgShowIndex(0);
                  } else {
                    setMsgShowIndex(showMsgIndex + 1);
                  }
                }}
              />
            </IconButton>
          </Stack>

          <img src={images[showMsgIndex]} style={{ width: "100%" }} />
        </Dialog>
      )}
    </Stack>
  );
}

function ChatHistory(props: {
  connName: string;
  handleClose: () => void;
  showChatHistory;
}) {
  const { handleClose, connName, showChatHistory } = props;
  const { messagesByNames, clearHistory, fetchHistory, isLoading } = useChatStore();
  const wrapperRef = useRef(null);
  const [matchStr, setMatchStr] = useState("");

  const [deBouncedMatch] = useDebounce(matchStr, 500);

  const history = messagesByNames[connName];
  const [page, setPage] = useState(1);
  const pageSize = 6;

  const { showingHis, length } = useMemo(() => {
    if (history) {
      let his = history.filter((his) =>
        deBouncedMatch
          ? his.content.toLocaleLowerCase().includes(deBouncedMatch.toLocaleLowerCase())
          : true,
      );
      return {
        showingHis: his.slice((page - 1) * pageSize, page * pageSize),
        length: his.length,
      };
    } else {
      return {
        showingHis: [],
        length: 0,
      };
    }
  }, [history, deBouncedMatch, page]);

  useEffect(() => {
    if (length > 0) {
      setPage(Math.ceil(length / pageSize));
    }
  }, [length]);

  useEffect(() => {
    const el = wrapperRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, []);

  useEffect(() => {
    fetchHistory(connName);
  }, [connName, fetchHistory]);
  const handleClear = () => {
    clearHistory(connName);
    handleClose();
  };
  if (!showChatHistory) {
    return <> </>;
  }

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
        display: "flex",
        flexDirection: "column",
      }}
      ref={wrapperRef}>
      <ModalHeader
        title="chat history"
        headerExtra={
          <>
            {isLoading && <CircularProgress />}
            <Tooltip title="delete history">
              <IconButton onClick={handleClear}>
                <DeleteForeverIcon />
              </IconButton>
            </Tooltip>
            <ClearableTextField
              value={matchStr}
              onChange={(e) => {
                setMatchStr(e.target.value);
              }}
              onClear={() => {
                setMatchStr("");
              }}
              label="search in history"
              style={{ width: "13rem" }}
            />
            {history && history.length ? (
              <Typography>
                {!matchStr
                  ? `${history.length} history`
                  : `showing ${length} of ${history.length}`}
              </Typography>
            ) : undefined}
          </>
        }
        handleClose={handleClose}
      />
      <div
        style={{
          flex: 1,
          height: "100px",
          minHeight: 0,
          overflow: "auto",
          paddingRight: "5px",
        }}>
        {showingHis && (
          <Stack>
            {showingHis.map((his, index) => {
              return (
                <div key={index}>
                  <Stack
                    direction="row"
                    style={{
                      justifyContent: his.role == "user" ? "flex-start" : "flex-end",
                      maxHeight: "37vh",
                      overflow: "auto",
                    }}>
                    <div
                      style={{
                        width: "85%",
                        overflow: "auto",
                      }}>
                      <Typography color="info">
                        {his.role}:{new Date(his.timestamp).toLocaleString()}
                      </Typography>
                      <AppMarkDown highlightStr={matchStr}>{his.content}</AppMarkDown>
                    </div>
                  </Stack>
                  {/* If have match string , I don't know the message is paired up */}
                  {his.role == "assistant" && <Divider />}
                </div>
              );
            })}
          </Stack>
        )}
      </div>
      <Stack direction="row" style={{ justifyContent: "right" }}>
        <Pagination
          page={page}
          onChange={(_, page) => {
            setPage(page);
          }}
          count={Math.ceil(length / pageSize)}
        />
      </Stack>
    </Paper>
  );
}

function EditPrompt(props: {
  connName: string;
  handleClose: () => void;
  showEdit: boolean;
}) {
  const { handleClose, connName, showEdit } = props;
  const { removePrompt, savePrompt, prompts } = useUserSystemPromptStore();
  const [value, setValue] = useState(prompts[connName] || "");
  const { showNotification } = useNotification();
  const handleDelete = () => {
    try {
      removePrompt(connName);
      showNotification({ severity: "success", title: "delete prompt success" });
      handleClose();
    } catch (e) {
      showNotification({
        severity: "error",
        title: "delete prompt failed",
        message: e,
      });
    }
  };

  useEffect(() => {
    setValue(prompts[connName] || "");
  }, [connName, prompts]);

  const handleSave = () => {
    try {
      savePrompt(connName, value);
      showNotification({ severity: "success", title: "save prompt success" });
    } catch (e) {
      showNotification({ severity: "error", title: "save prompt failed", message: e });
    }
  };
  if (!showEdit) {
    return <> </>;
  }
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
        display: showEdit ? "flex" : "none",
        flexDirection: "column",
      }}>
      <ModalHeader
        title="edit prompt"
        headerExtra={
          <>
            <Tooltip title="save this prompt">
              <IconButton onClick={handleSave}>
                <SaveIcon />
              </IconButton>
            </Tooltip>
            <Tooltip title="delete this prompt ">
              <IconButton onClick={handleDelete}>
                <DeleteForeverIcon />
              </IconButton>
            </Tooltip>
          </>
        }
        handleClose={handleClose}
      />

      <SimpleFlexEditor
        language="text"
        value={value}
        handleValue={(v) => {
          setValue(v);
        }}
      />
    </Paper>
  );
}

export { AllKindChat };
