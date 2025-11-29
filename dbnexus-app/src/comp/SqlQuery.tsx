import { Editor } from "@monaco-editor/react";
import { MultiQueryResp, QueryRespData, queryRespFix } from "./QueryResp";
import EventNoteIcon from "@mui/icons-material/EventNote";
import {
  Box,
  Checkbox,
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
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
  QueryConn,
  QueryEvent,
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
  ErrAndLoc,
  generateRandomString,
  getEditorText,
  getLanguageFromDataSourceKind,
  MuiDataGridData,
  QueryLogHandler,
  resizeHandlerSize,
  rustResultOk,
  SaveQueryLogConfigure,
  secondConfirm,
  SetCloseConfirmTextContext,
  SettingContext,
  shrinkAble,
} from "./helper.ts";
import ArrowCircleDownRoundedIcon from "@mui/icons-material/ArrowCircleDownRounded";
import { ExportView } from "./Export.tsx";
import { ExecutingTime, PossibleParseError } from "./CommonComp.tsx";
import {
  CancelButton,
  EditorBar,
  LoadStoreFile,
  SaveFavoriteSqlView,
} from "./CommonComp2.tsx";
import { ModifyLLMConfigModal } from "./Settings.tsx";
import { cancelTask, checkSql } from "./handlers.ts";
import { Channel } from "@tauri-apps/api/core";
function SqlTransaction(props: { conn: QueryConn; executeTimes: number }) {
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

class SaveCheckSql {
  static key = "dbpal-save-check-sql";
  static get(): boolean {
    return localStorage.getItem(SaveCheckSql.key) == "1";
  }
  static save(x: boolean) {
    localStorage.setItem(SaveCheckSql.key, x ? "1" : "0");
  }
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

  const [resp, setResp] = useState<QueryRespData[]>([]);
  const [sqlErr, setSqlErr] = useState<ErrAndLoc | null>(null);
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position | null>(null);
  const [loading, setLoading] = useState(false);
  const [makeConnLoading, setMakeConnLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [showQueryLog, setShowQueryLog] = useState(false);
  const setting = useContext(SettingContext);
  const queryLogHandler = useRef(new QueryLogHandler(uuid));
  const [executeTimes, setExecuteTimes] = useState(0);
  const [conn] = useState<QueryConn>(
    new QueryConn(uuid, generateRandomString(8), kind),
  );
  const [showExport, setShowExport] = useState(false);
  const [openOk, setOpenOk] = useState(false);
  const [tryCheckSql, setTryCheckSql] = useState(SaveCheckSql.get());
  useEffect(() => {
    SaveCheckSql.save(tryCheckSql);
  }, [tryCheckSql]);

  const handleEditorDidMount = useCallback((ed: editor.IStandaloneCodeEditor) => {
    setEditor(ed);
    const d1 = ed.onDidChangeCursorPosition(() => {
      const p = ed.getPosition();
      setPos(p);
    });
    return () => {
      d1.dispose();
    };
  }, []);

  useEffect(() => {
    if (!editor) {
      return;
    }
    if (handleEditor) {
      handleEditor(editor);
    }
    let x = editor.getModel().onDidChangeContent(() => {
      if (!tryCheckSql) {
        setSqlErr(null);
        return;
      }
      checkSql(uuid, editor.getValue()).then(e => {
        setSqlErr(e);
      });
    });
    return () => {
      x.dispose();
    };
  }, [editor, handleEditor, tryCheckSql, uuid]);

  useEffect(() => {
    setMakeConnLoading(true);
    conn
      .open()
      .then(() => {
        setOpenOk(true);
      })
      .catch(e => {
        showNotification("error", "establish connection failed", e);
      })
      .finally(() => {
        setMakeConnLoading(false);
      });
  }, [conn, showNotification]);

  useEffect(() => {
    return () => {
      conn.destroy();
    };
  }, [conn]);
  const handleReconnect = async () => {
    setMakeConnLoading(true);
    conn
      .open()
      .then(() => {
        showNotification("success", "Reconnect OK");
      })
      .catch(e => {
        showNotification("error", "Reconnect Failure", e);
      })
      .finally(() => {
        // trigger some reload like sqltransaction
        setExecuteTimes(e => e + 1);
        setMakeConnLoading(false);
      });
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
        if (e.ok) {
          queryRespFix(e.ok);
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
  const handleQuery = async (append: boolean, explain: boolean = false) => {
    let text = getEditorText(editor, showNotification);
    if (!text) {
      return;
    }
    if (explain) {
      text = `EXPLAIN ${text}`;
    }
    console.log("handle query");
    if (!append) {
      setResp([]);
    }
    const confirm = await warnDangerousSql(text);
    if (!confirm) {
      return;
    }
    setLoading(true);
    let emit = new Channel<QueryEvent>(e => {
      // In Strict Mode, React will call your initializer function twice in order to help you find accidental impurities. This is development-only behavior and does not affect production. If your initializer function is pure (as it should be), this should not affect the behavior. The result from one of the calls will be ignored.
      setResp(resp => {
        return appendQueryResult(resp, e, showNotification);
      });
    });
    conn
      .queryOP(text, emit)
      .then(() => {
        if (SaveQueryLogConfigure.get()) {
          queryLogHandler.current.save(text, true);
        }
        showNotification("success", "Execute OK");
      })
      .catch(e => {
        if (SaveQueryLogConfigure.get()) {
          queryLogHandler.current.save(text, false);
        }
        showNotification("error", "query failed", e);
      })
      .finally(() => {
        setExecuteTimes(e => e + 1);
        setLoading(false);
      });
  };

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
            queryId={conn.queryId}
          />
          <Stack
            direction="row"
            sx={{
              alignItems: "center",
              overflow: "auto",
              paddingBottom: "0.7rem",
            }}
          >
            {openOk && (
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
                {loading && (
                  <Tooltip title="cancel this operation">
                    <CancelButton
                      handleCancel={() => {
                        cancelTask(conn.queryId);
                      }}
                    />
                  </Tooltip>
                )}
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
                <Tooltip title="try check sql,right now implemented it using parser">
                  <Checkbox
                    checked={tryCheckSql}
                    onChange={e => {
                      setTryCheckSql(e.target.checked);
                    }}
                    name="myCheckbox"
                    color="primary"
                  />
                </Tooltip>
              </>
            )}
            {openOk && conn.supportTransaction() && (
              <SqlTransaction executeTimes={executeTimes} conn={conn} />
            )}

            {openOk && (
              <Tooltip title={"reconnect"}>
                <IconButton onClick={handleReconnect} color="primary">
                  <RefreshIcon />
                </IconButton>
              </Tooltip>
            )}

            {openOk && (
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

            {openOk && (
              <Tooltip title="connection id">
                <Typography style={shrinkAble}> #{conn.queryId} </Typography>
              </Tooltip>
            )}
            {makeConnLoading && <CircularProgress />}
            <ModifyLLMConfigModal />
            <ExecutingTime loading={loading} disableTime />
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
        {resp.length > 0 && (
          <PanelResizeHandle
            style={{ height: resizeHandlerSize }}
            className="resize-handle"
          />
        )}

        {resp.length > 0 && (
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
              exportOP={async (kind, file, taskId, sizeEmitter) => {
                const sql = getEditorText(editor, showNotification);
                return conn.exportQueryResult(
                  conn.queryId,
                  sql,
                  kind,
                  file,
                  taskId,
                  sizeEmitter,
                );
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

function appendQueryResult(
  data: QueryRespData[],
  event: QueryEvent,
  showNotification,
): QueryRespData[] {
  switch (event.kind) {
    case "structure":
      return [...data, { ok: event.resp }];
    case "err":
      return [...data, { err: event.err }];
    case "data": {
      if (event.data.length == 0) {
        showNotification("warning", "backend send empty data");
        return data;
      }
      let data0 = event.data[0];
      showNotification(
        "info",
        `backend send ${event.data.length} record,data:${data0.slice(0, 3).join("-")}`,
      );
      let index = null;
      for (let i = data.length - 1; i >= 0; --i) {
        if (typeof data[i] == "object") {
          index = i;
          break;
        }
      }
      if (index === null) {
        showNotification(
          "error",
          "should be able to find the index the place data ,this is internal error",
        );
        return data;
      }
      let cop = { ...data[index].ok } as MuiDataGridData;
      cop.data = [
        ...cop.data,
        ...event.data.map(d => {
          let obj = {};
          d.forEach((e, index) => {
            obj[cop.fields[index].field] = e;
          });
          return obj;
        }),
      ];
      let arr = [...data];
      queryRespFix(cop);
      arr[index].ok = cop;
      return arr;
    }
    case "status": {
      showNotification(
        "info",
        "server is dothing this.",
        <Stack direction="row">
          <CircularProgress />
          <Typography> {event.status} </Typography>
        </Stack>,
      );
      return data;
    }
    default:
      showNotification(
        "error",
        "backend give some data which we can not handle",
        event,
      );
  }
}

export { SqlQuery };
