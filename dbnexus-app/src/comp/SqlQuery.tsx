"use strict";
import { Editor } from "@monaco-editor/react";
import { MultiQueryResp } from "./QueryResp";
import EventNoteIcon from "@mui/icons-material/EventNote";
import {
  Box,
  Chip,
  CircularProgress,
  IconButton,
  Modal,
  Stack,
  Tooltip,
  Typography,
} from "@mui/material";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";

import PlayArrowIcon from "@mui/icons-material/PlayArrow";
import { FaExclamation } from "react-icons/fa";
import {
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import FastForwardIcon from "@mui/icons-material/FastForward";
import { editor, Position } from "monaco-editor";
import {
  EscDownEvent,
  IQueryConn,
  makeQueryConn,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
  SetAutoCompletionName,
} from "./Common.ts";
import { BsFillLightbulbFill } from "react-icons/bs";
import { FaUpload } from "react-icons/fa";
import { MdOutlineReply } from "react-icons/md";
import { NotificationContext } from "./Notification.tsx";
import { QueryLog } from "./QueryLog.tsx";
import RefreshIcon from "@mui/icons-material/Refresh";
import {
  DataSourceKind,
  DetachQueyRespViewParameter,
  ErrAndLoc,
  generateRandomString,
  getEditorText,
  getLanguageFromDataSourceKind,
  QueryLogHandler,
  rustResultOk,
  SaveQueryLogConfigure,
  secondConfirm,
  SetCloseConfirmTextContext,
  SettingContext,
  shrinkAble,
  SomeTempStore,
} from "./helper.ts";
import ArrowCircleDownRoundedIcon from "@mui/icons-material/ArrowCircleDownRounded";
import { ExportView } from "./Export.tsx";
import { ExecutingTime, PossibleParseError } from "./CommonComp.tsx";
import { EditorBar, LoadStoreFile, SaveFavoriteSqlView } from "./CommonComp2.tsx";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { ModifyLLMConfigModal } from "./Settings.tsx";
function SqlTransaction(props: { conn: IQueryConn; executeTimes: number }) {
  const { conn, executeTimes } = props;
  const [commit, setCommit] = useState(true);
  const { showNotification } = useContext(NotificationContext);
  const [txCount, setTxCount] = useState(0);
  const setCloseConfirm = useContext(SetCloseConfirmTextContext);
  const handleQueryCommit = useCallback(async () => {
    try {
      const c = await conn.getAutoCommit();
      setCommit(c);
    } catch (e) {
      showNotification("error", "get commit model failed", e);
    }
  }, [conn, showNotification]);
  useEffect(() => {
    setTxCount(0);
    if (commit) {
      setCloseConfirm("");
    } else {
      setCloseConfirm(
        "In Transaction,better commit or rollback.uncommit transaction may lead database locks",
      );
    }
  }, [commit, setCloseConfirm]);

  useEffect(() => {
    //这里的顺序很重要
    setTxCount(e => e + 1);
    handleQueryCommit();
  }, [executeTimes, handleQueryCommit]);

  const commitTx = async () => {
    try {
      const confirm = await secondConfirm({
        title: "Commit Transaction",
        confirmText: "Commit",
        kind: "info",
      });
      if (!confirm) {
        return;
      }
      await conn.commit();
      showNotification("success", "commit ok");
      handleQueryCommit();
    } catch (e) {
      showNotification("error", "commit failed", e);
    }
  };

  const rollbackTx = async () => {
    const confirm = await secondConfirm({
      title: "Rollback Transaction",
      confirmText: "Rollback",
      kind: "info",
    });
    if (!confirm) {
      return;
    }
    try {
      await conn.rollback();
      showNotification("success", "rollback ok");
      handleQueryCommit();
    } catch (e) {
      showNotification("error", "rollback failed", e);
    }
  };
  const reverseCommitMode = async () => {
    try {
      await conn.setAutoCommit(!commit);
      setCommit(e => !e);
      showNotification("success", "set commit mode ok");
    } catch (e) {
      showNotification("error", "set commit failed", e);
    }
  };

  return (
    <>
      <Tooltip title="reverse commit mode">
        <Chip
          onClick={reverseCommitMode}
          label={`${commit ? "auto" : "manual"}-commit`}
          color={commit ? "info" : "success"}
          variant={commit ? "outlined" : "filled"}
          style={shrinkAble}
        />
      </Tooltip>

      {!commit && (
        <>
          <Tooltip title="executes after begin transaction">
            <Chip label={txCount} color="info" />
          </Tooltip>

          <Tooltip title="commit">
            <IconButton onClick={commitTx} color="primary">
              <FaUpload fontSize="1.1rem" />
            </IconButton>
          </Tooltip>
          <Tooltip title="rollback">
            <IconButton onClick={rollbackTx} color="primary">
              <MdOutlineReply fontSize="1.7rem" />
            </IconButton>
          </Tooltip>
        </>
      )}
    </>
  );
}

function SqlQuery(props: {
  uuid: string;
  value?: string;
  favoriteName?: string;
  kind: DataSourceKind;
  extras?: ReactNode;
  handleEditor: (ed: editor.IStandaloneCodeEditor) => void;
}) {
  const { uuid, value, favoriteName, kind, extras, handleEditor } = props;
  const makeConn = useCallback(async () => {
    return makeQueryConn(uuid, kind);
  }, [kind, uuid]);
  const [resp, setResp] = useState([]);
  const [sqlErr] = useState<ErrAndLoc | null>(null);
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position | null>(null);
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [showQueryLog, setShowQueryLog] = useState(false);
  const setting = useContext(SettingContext);
  const queryLogHandler = useRef(new QueryLogHandler(uuid));
  const [executeTimes, setExecuteTimes] = useState(0);
  const [conn, setConn] = useState<IQueryConn | null>(null);
  const [showExport, setShowExport] = useState(false);
  const [queryRespView, setQueryRespView] = useState<WebviewWindow>(null);
  const queryRespViewEventName = useRef(generateRandomString());
  const handleEditorDidMount = useCallback(
    (ed: editor.IStandaloneCodeEditor) => {
      setEditor(ed);
      if (handleEditor) {
        handleEditor(ed);
      }
      ed.onDidChangeCursorPosition(() => {
        const p = ed.getPosition();
        setPos(p);
      });
    },
    [handleEditor],
  );

  const handleReconnect = async () => {
    if (conn) {
      const confirm = await secondConfirm({
        title: "reconnection",
        content: "unsaved data may lost.",
        confirmText: "Connect",
        kind: "info",
      });
      if (!confirm) {
        return;
      }
    }
    if (conn) {
      conn
        .reconnect()
        .then(conn => {
          setConn(conn);
          if (queryRespView) {
            // send new query id
            queryRespView.setTitle(makeWindowTitle(uuid, conn.queryId)).catch(e => {
              showNotification("error", "set query response view title failed", e);
            });
            const payload = {};
            payload[QuerySeparateWindowMsg.newQueryIDName] = conn.queryId;
            queryRespView.emit(queryRespViewEventName.current, payload).catch(e => {
              showNotification("error", "send new query id failed", e);
            });
          }
          showNotification("success", "Reconnect OK");
        })
        .catch(e => {
          showNotification("error", "Reconnect Failure", e);
        })
        .finally(() => {
          // trigger some reload like sqltransaction
          setExecuteTimes(e => e + 1);
        });
    } else {
      makeConn()
        .then(conn => {
          setConn(conn);
          showNotification("success", "Reconnect OK");
        })
        .catch(e => {
          showNotification("error", "Reconnect Failure", e);
        })
        .finally(() => {
          setExecuteTimes(e => e + 1);
        });
    }
  };
  const warnDangerousSql = useCallback(
    async (text: string): Promise<boolean> => {
      try {
        if (await conn.isDangerSql(text)) {
          return await secondConfirm({
            confirmText: "Execute",
            title: "This is a dangerous sql",
            content: `Are u sure going to execute!!!!'`,
          });
        } else {
          return true;
        }
      } catch (e) {
        if (import.meta.env.DEV) {
          console.log("warnDangerousSql error", e);
        }

        return true;
      }
    },
    [conn],
  );

  const handleExecute = async () => {
    const text = getEditorText(editor, showNotification);
    if (!text) {
      return;
    }
    const confirm = await warnDangerousSql(text);
    if (!confirm) {
      return;
    }
    setLoading(true);
    conn
      .executeOP(text)
      .then(e => {
        if (SaveQueryLogConfigure.get()) {
          queryLogHandler.current.save(text, rustResultOk(e));
        }
        setResp([e]);
        showNotification("success", "execute ok");
      })
      .catch(e => {
        setResp([]);
        showNotification("error", "query failed", e);
      })
      .finally(() => {
        setExecuteTimes(e => e + 1);
        setLoading(false);
      });
  };
  const handleQuery = useCallback(
    async (append: boolean, explain: boolean = false) => {
      let text = getEditorText(editor, showNotification);
      if (!text) {
        return;
      }
      if (explain) {
        text = `EXPLAIN ${text}`;
      }
      const confirm = await warnDangerousSql(text);
      if (!confirm) {
        return;
      }
      setLoading(true);
      conn
        .queryOP(text, append)
        .then(resp => {
          if (SaveQueryLogConfigure.get()) {
            queryLogHandler.current.save(text, rustResultOk(resp));
          }
          if (append) {
            setResp(p => [...p, ...resp]);
          } else {
            setResp(resp);
          }
          showNotification("success", "Execute OK");
        })
        .catch(e => {
          setResp([]);
          showNotification("error", "query failed", e);
        })
        .finally(() => {
          setExecuteTimes(e => e + 1);
          setLoading(false);
        });
    },
    [conn, editor, showNotification, warnDangerousSql],
  );

  // make connection for the first time
  useEffect(() => {
    makeConn()
      .then(conn => {
        setConn(prev => {
          if (prev) {
            prev.destroy();
          }
          return conn;
        });
      })
      .catch(e => {
        showNotification("error", "make connection failed", e);
      });
  }, [makeConn, showNotification]);
  // destroy connection
  useEffect(() => {
    return () => {
      if (conn) {
        console.log("destry connection", conn.queryId);
        conn.destroy();
      }
    };
  }, [conn]);
  useEffect(() => {
    return () => {
      if (queryRespView) {
        queryRespView.close();
      }
    };
  }, [queryRespView]);
  // destroy query response view window
  useEffect(() => {
    if (!queryRespView) {
      return;
    }
    const unlisten = queryRespView.onCloseRequested(() => {
      setQueryRespView(null);
    });
    return () => {
      unlisten.then(e => {
        e();
      });
    };
  }, [queryRespView]);

  // send data to query response view
  useEffect(() => {
    if (!queryRespView) {
      return;
    }
    queryRespView
      .emit(queryRespViewEventName.current, QuerySeparateWindowMsg.reloadData)
      .catch(e => {
        showNotification("error", "emit query response failed", e);
      });
  }, [queryRespView, resp, showNotification]);

  const editorOptions = useMemo((): editor.IStandaloneEditorConstructionOptions => {
    return {
      fontSize: setting.appearance.fontSize,
      fontWeight: setting.appearance.fontWeight,
      fontFamily: setting.appearance.fontFamily,
      tabSize: setting.editor.tabSize,
      ...setting.editor.editorOption,
    };
  }, [setting]);

  const editorDivDom = useRef(null);
  useEffect(() => {
    if (editorDivDom.current && conn) {
      (editorDivDom.current as HTMLDivElement).addEventListener(
        "keydown",
        function (event) {
          if (event.ctrlKey && event.key.toLowerCase() === "e") {
            event.preventDefault();
            handleQuery(false);
          }
        },
      );
    }
  }, [conn, handleQuery]);
  useEffect(() => {
    if (conn && editor) {
      SetAutoCompletionName.set(
        editor.getModel().id,
        new NameAndQueryId(uuid, kind, conn.queryId),
      );
    }
  }, [conn, editor, kind, uuid]);

  const supportExplain = [
    DataSourceKind.Sqlite,
    DataSourceKind.LibSql,
    DataSourceKind.MariaDB,
    DataSourceKind.TiDB,
    DataSourceKind.Mysql,
    DataSourceKind.Oracle,
    DataSourceKind.SqlServer,
    DataSourceKind.Postgres,
    DataSourceKind.ClickHouse,
    DataSourceKind.FireBird,
  ].includes(kind);

  const toggleQueryRespView = (maximized: boolean, x?: number, y?: number) => {
    if (queryRespView) {
      try {
        queryRespView.close();
      } catch (e) {
        console.error(e);
        return;
      }
    } else {
      SomeTempStore.set({
        uuid,
        queryId: conn.queryId,
        eventName: queryRespViewEventName.current,
      } as DetachQueyRespViewParameter);
      const webview = new WebviewWindow(generateRandomString(), {
        title: makeWindowTitle(uuid, conn.queryId),
        url: "query_response.html",
        width: 800,
        height: 600,
        maximized,
        x,
        y,
      });
      webview.once("tauri://created", () => {
        console.info("create query response view successfully");
      });
      webview.once(queryRespViewEventName.current, e => {
        console.info("query window send me something:", e.payload);
        if (e.payload == QuerySeparateWindowMsg.listenOK) {
          setQueryRespView(webview);
        } else {
          console.log("query window send me something wrong:", e.payload);
        }
      });
      webview.once("tauri://error", e => {
        showNotification("error", "create webview failed", e.event.toString());
      });
    }
  };

  return (
    <div
      style={{
        height: "100%",
        overflow: "auto",
        display: "flex",
        flexDirection: "column",
        marginBottom: "300px",
      }}
    >
      <PanelGroup direction="vertical" style={{ flex: 1 }} autoSaveId="sql-query">
        <Panel
          minSize={10}
          maxSize={100}
          defaultSize={40}
          style={{
            display: "flex",
            flexDirection: "column",
          }}
          collapsible
          collapsedSize={0}
        >
          <QueryLog
            showQueryLog={showQueryLog}
            setShowQueryLog={setShowQueryLog}
            uuid={uuid}
            executeTimes={executeTimes}
            kind={kind}
            queryId={conn?.queryId}
          />

          <Stack
            direction="row"
            sx={{
              alignItems: "center",
              overflow: "auto",
              paddingBottom: "0.7rem",
            }}
          >
            {conn && (
              <>
                <Tooltip title="query">
                  <IconButton
                    loading={loading}
                    disabled={loading}
                    onClick={() => {
                      handleQuery(false);
                    }}
                    color="primary"
                  >
                    <PlayArrowIcon />
                  </IconButton>
                </Tooltip>

                <Tooltip title="query and append result to results tabs">
                  <IconButton
                    loading={loading}
                    disabled={loading}
                    onClick={() => {
                      handleQuery(true);
                    }}
                    color="primary"
                  >
                    <FastForwardIcon />
                  </IconButton>
                </Tooltip>

                <Tooltip title="execute sql not expecting result">
                  <IconButton
                    loading={loading}
                    disabled={loading}
                    onClick={handleExecute}
                    color="primary"
                  >
                    <FaExclamation />
                  </IconButton>
                </Tooltip>
                {supportExplain && (
                  <Tooltip title="explain sql">
                    <IconButton
                      loading={loading}
                      disabled={loading}
                      onClick={() => {
                        handleQuery(false, true);
                      }}
                      color="primary"
                    >
                      <BsFillLightbulbFill />
                    </IconButton>
                  </Tooltip>
                )}
              </>
            )}

            {conn && conn.supportTransaction() && (
              <SqlTransaction executeTimes={executeTimes} conn={conn} />
            )}
            <Tooltip title={"reconnect"}>
              <IconButton
                onClick={() => {
                  handleReconnect();
                }}
                color="primary"
              >
                <RefreshIcon />
              </IconButton>
            </Tooltip>
            {conn && (
              <Tooltip title="export query result to a file,implemented it not using query connection,uncommit row may not be found">
                <IconButton
                  loading={loading}
                  disabled={loading}
                  onClick={() => {
                    const value = getEditorText(editor, showNotification);
                    if (!value) {
                      return;
                    }
                    setShowExport(true);
                  }}
                  color="primary"
                >
                  <ArrowCircleDownRoundedIcon />
                </IconButton>
              </Tooltip>
            )}
            <SaveFavoriteSqlView
              favoriteName={favoriteName}
              uuid={uuid}
              kind={kind}
              editor={editor}
            />
            {editor && (
              <LoadStoreFile
                getQuery={() => {
                  return editor.getValue();
                }}
                handleLoadFile={(_, content) => {
                  editor.setValue(content);
                }}
                kind={kind}
              />
            )}

            <Tooltip title="query log">
              <IconButton
                onClick={() => {
                  setShowQueryLog(prev => !prev);
                }}
                color="primary"
              >
                <EventNoteIcon />
              </IconButton>
            </Tooltip>
            {extras}

            {conn && (
              <Tooltip
                title={
                  queryRespView
                    ? "close query response view"
                    : "can open the query results in a separate window."
                }
              >
                <Chip
                  variant={queryRespView ? "outlined" : "filled"}
                  onClick={() => {
                    toggleQueryRespView(true);
                  }}
                  style={shrinkAble}
                  label={queryRespView ? "close response window" : "detach response"}
                />
              </Tooltip>
            )}
            {conn && (
              <Tooltip title="connection id">
                <Typography> #{conn.queryId} </Typography>
              </Tooltip>
            )}
            {!conn && <CircularProgress />}
            <ExecutingTime loading={loading} disableTime />

            <ModifyLLMConfigModal />
          </Stack>
          <div
            style={{
              flex: 1,
              minHeight: 0,
              height: "100px",
            }}
            ref={editorDivDom}
          >
            <Editor
              language={getLanguageFromDataSourceKind(kind)}
              onMount={handleEditorDidMount}
              width="99%"
              height="99%"
              value={value}
              options={editorOptions}
              theme={setting.editorTheme()}
            />
          </div>
          <EditorBar
            left={
              <Stack
                style={{
                  flex: 1,
                }}
                direction="row"
              >
                {sqlErr && <PossibleParseError parseErr={sqlErr} editor={editor} />}
              </Stack>
            }
            pos={pos}
          />
        </Panel>
        {!queryRespView && resp.length > 0 && (
          <PanelResizeHandle style={{ height: 4 }} className="resize-handle" />
        )}

        {!queryRespView && resp.length > 0 && (
          <Panel
            style={{ overflow: "auto" }}
            collapsible
            collapsedSize={0}
            maxSize={100}
            minSize={10}
          >
            <div style={{ height: "100%" }}>
              <MultiQueryResp data={resp} />
            </div>
          </Panel>
        )}
      </PanelGroup>

      {showExport && (
        <Modal
          open={showExport}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowExport(false);
          })}
        >
          <Box sx={modalStyle} width="45%">
            <ExportView
              supportKind={["csv", "json", "jsonline", "xlsx", "html"]}
              exportOP={async (kind, file) => {
                const sql = getEditorText(editor, showNotification);
                return conn.exportQueryResult(sql, kind, file);
              }}
              handleClose={() => {
                setShowExport(false);
              }}
              name="export_query_result"
            />
          </Box>
        </Modal>
      )}
    </div>
  );
}

class QuerySeparateWindowMsg {
  static listenOK = "listen_ok";
  static newQueryIDName = "new_query_id";
  static reloadData = "reload_data";
}

function makeWindowTitle(uuid, queryid): string {
  return `query result ${uuid}#${queryid}`;
}

export { SqlQuery, QuerySeparateWindowMsg };
