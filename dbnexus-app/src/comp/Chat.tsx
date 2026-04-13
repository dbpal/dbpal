import {
  ChatShowContext,
  DataSourceKind,
  extractConfigNameFromUuid,
  QueryIDEditorChatCtx,
} from "./helper.ts";
import { v4 as uuidv4 } from "uuid";
import CloseIcon from "@mui/icons-material/Close";
import SaveIcon from "@mui/icons-material/Save";
import { TbPrompt } from "react-icons/tb";
import {
  Box,
  Checkbox,
  CircularProgress,
  Divider,
  FormControlLabel,
  IconButton,
  Modal,
  Pagination,
  Paper,
  Stack,
  Tooltip,
  Typography,
} from "@mui/material";
import { useContext, useEffect, useMemo, useRef, useState } from "react";
import { editor } from "monaco-editor";
import {
  SliderSize,
  SimpleFlexEditor,
  CancelButton,
  EscPressedDo,
  ClearableTextField,
  AppMarkDown,
} from "./CommonComp2.tsx";
import ArrowCircleUpIcon from "@mui/icons-material/ArrowCircleUp";
import EventNoteIcon from "@mui/icons-material/EventNote";
import {
  convertPathToDataUrl,
  dbpalQuerykey,
  editorAppendContent,
  EscDownEvent,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
} from "./Common.ts";
import { ModalHeader } from "./CommonComp.tsx";
import { AllKindQuery } from "./Left.tsx";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import NavigateNextIcon from "@mui/icons-material/NavigateNext";
import NavigateBeforeIcon from "@mui/icons-material/NavigateBefore";

import {
  useBoolSwitch,
  useChatStore,
  useLLM,
  useNotification,
  useUserSystemPromptStore,
  UseSizeConfig,
  useSizeStore,
} from "./store.tsx";

import AddPhotoAlternateIcon from "@mui/icons-material/AddPhotoAlternate";
import { open } from "@tauri-apps/plugin-dialog";

import { useDebounce } from "use-debounce";

function AllKindChat(props: {
  uuid: string;
  kind: DataSourceKind;
  value?: string;
  favoriteName?: string;
  language?: string; // mongo used this as javascript or typescript
}) {
  const { uuid, kind, favoriteName, value, language } = props;
  const llm = useLLM();
  const showChatKey = "chat-show-" + uuid;
  const { getSwitch, setSwitch } = useBoolSwitch();
  const [show, setShow] = useState(getSwitch(showChatKey));

  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  useEffect(() => {
    setSwitch(showChatKey, show);
  }, [setSwitch, show, showChatKey]);

  // use UUID
  let queryId = useRef(uuidv4());
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
          <AllKindQuery
            uuid={uuid}
            kind={kind}
            favoriteName={favoriteName}
            value={value}
            language={language}
          />
          {llm && editor && <SqlChatTextInput uuid={uuid} editor={editor} />}
        </ChatShowContext.Provider>
      </Stack>
    </QueryIDEditorChatCtx.Provider>
  );
}

const cleanEditorKey = "clean-editor-key";

function SqlChatTextInput(props: {
  uuid: string;
  editor: editor.IStandaloneCodeEditor;
}) {
  const { editor, uuid } = props;
  const llm = useLLM();
  const [question, setQuestion] = useState("");
  const { showNotification } = useNotification();
  const [showChatHistory, setShowChatHistory] = useState(false);
  const [showEditPrompt, setShowEditPrompt] = useState(false);
  const [invokeStatus, setInvokeStatus] = useState("");
  const [cancelFN, setCancelFN] = useState<() => void>(null);
  const { show, setShow } = useContext(ChatShowContext);
  const { getSwitch, setSwitch } = useBoolSwitch();
  const cleanEditor = getSwitch(cleanEditorKey);
  const [images, setImages] = useState<string[]>([]);
  const { getSize } = useSizeStore();
  const { messagesByNames, saveMessage } = useChatStore();
  const connName = extractConfigNameFromUuid(uuid);
  const { prompts: userDefinePrompts } = useUserSystemPromptStore();

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
      setInvokeStatus("making prompt");
      let wholeText = "";
      setInvokeStatus("trying getting stream response");
      let i = 0;
      let queryId = editor.getModel()[dbpalQuerykey] as NameAndQueryId;
      saveMessage(connName, "user", question);
      const history = messagesByNames[connName];
      await llm.stream(
        queryId.name,
        queryId.queryId,
        question,
        images,
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
                    setImages([...images, fileContent]);
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
          <ShowChatImages images={images} setImages={setImages} />
        </Stack>

        <Divider> </Divider>
        <SimpleFlexEditor
          language="text"
          barLeft={invokeStatus && <Typography color="info">{invokeStatus}</Typography>}
          handleValue={(v) => {
            setQuestion(v);
          }}
          handleEditor={(e) => {
            e.getModel()[dbpalQuerykey] = { ...editor.getModel()[dbpalQuerykey] };
          }}
          dontFocus
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
        <Modal
          open={show}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setMsgShowIndex(null);
          })}>
          <Box sx={{ ...modalStyle, width: "80vw", height: "60vh" }}>
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
          </Box>
        </Modal>
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
            <Typography>
              {!matchStr
                ? `${history.length} history`
                : `showing ${length} of ${history.length}`}
            </Typography>
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

  useEffect(() => {
    console.log("current edit prompt value", value);
  }, [value]);

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
