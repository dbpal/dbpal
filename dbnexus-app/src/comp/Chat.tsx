import {
  ChatShowContext,
  DataSourceKind,
  extractConfigNameFromUuid,
  TabActiveContext,
} from "./helper.ts";
import CloseIcon from "@mui/icons-material/Close";
import {
  Box,
  Checkbox,
  Divider,
  FormControlLabel,
  IconButton,
  Modal,
  Paper,
  Stack,
  Tooltip,
  Typography,
} from "@mui/material";
import { useContext, useEffect, useRef, useState } from "react";
import { editor } from "monaco-editor";
import {
  AppMarkDown,
  SliderSize,
  SimpleFlexEditor,
  CancelButton,
} from "./CommonComp2.tsx";
import ArrowCircleUpIcon from "@mui/icons-material/ArrowCircleUp";
import EventNoteIcon from "@mui/icons-material/EventNote";
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
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import NavigateNextIcon from "@mui/icons-material/NavigateNext";
import NavigateBeforeIcon from "@mui/icons-material/NavigateBefore";

import {
  useBoolSwitch,
  useChatStore,
  useLLM,
  useNotification,
  UseSizeConfig,
  useSizeStore,
} from "./store.tsx";

import AddPhotoAlternateIcon from "@mui/icons-material/AddPhotoAlternate";
import { open } from "@tauri-apps/plugin-dialog";

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

const cleanEditorKey = "clean-editor-key";

function SqlChatTextInput(props: {
  uuid: string;
  kind: DataSourceKind;
  editor: editor.IStandaloneCodeEditor;
}) {
  const { editor, uuid } = props;
  const llm = useLLM();
  const [question, setQuestion] = useState("");
  const { showNotification } = useNotification();
  const [showChatHistory, setShowChatHistory] = useState(false);
  const [invokeStatus, setInvokeStatus] = useState("");
  const [cancelFN, setCancelFN] = useState<() => void>(null);
  const { show, setShow } = useContext(ChatShowContext);
  const { getSwitch, setSwitch } = useBoolSwitch();
  const cleanEditor = getSwitch(cleanEditorKey);
  const [images, setImages] = useState<string[]>([]);
  const vis = useContext(TabActiveContext);
  const { getSize } = useSizeStore();
  const { messagesByNames, saveMessage } = useChatStore();
  const connName = extractConfigNameFromUuid(uuid);

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
      setShowChatHistory(false);
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
      let queryId = SetAutoCompletionName.get(editor.getModel().id);
      saveMessage(connName, "user", question);
      const history = messagesByNames[connName];
      await llm.stream(
        queryId.name,
        queryId.queryId,
        question,
        images,

        (function () {
          if (!history) {
            return [];
          }
          return history.slice(0, getSize(UseSizeConfig.chatMemoryKey)).map((h) => {
            return {
              role: h.role,
              msg: h.content,
            };
          });
        })(),

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
      saveMessage(connName, "assistant", wholeText);
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
                    showNotification("error", "image have already been add");
                  } else {
                    setImages([...images, fileContent]);
                  }
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
          connName={connName}
          handleClose={() => {
            setShowChatHistory(false);
          }}
        />
      )}
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
          <Box sx={{ ...modalStyle }} width="80vw" height="60vh">
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

function ChatHistory(props: { connName: string; handleClose: () => void }) {
  const { handleClose, connName } = props;

  const { messagesByNames, clearHistory } = useChatStore();
  const wrapperRef = useRef(null);
  useEffect(() => {
    const el = wrapperRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, []);

  const history = messagesByNames[connName];

  const handleClear = () => {
    clearHistory(connName);
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
              <DeleteForeverIcon />
            </IconButton>
          </Tooltip>
        }
        handleClose={handleClose}
      />
      {history && (
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
                    <AppMarkDown>{his.content}</AppMarkDown>
                  </div>
                </Stack>
                {index > 0 && (index + 1) % 2 == 0 && <Divider />}
              </div>
            );
          })}
        </Stack>
      )}
    </Paper>
  );
}

export { Chat as AllKindChat };
