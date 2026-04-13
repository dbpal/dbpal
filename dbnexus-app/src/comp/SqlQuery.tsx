import { MultiQueryResp, QueryRespData } from "./QueryResp";
import EventNoteIcon from "@mui/icons-material/EventNote";
import {
  Box,
  Checkbox,
  Chip,
  Divider,
  IconButton,
  Modal,
  Stack,
  Tooltip,
} from "@mui/material";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import PlayArrowIcon from "@mui/icons-material/PlayArrow";
import { FaExclamation } from "react-icons/fa";
import { ReactNode, useCallback, useContext, useEffect, useRef, useState } from "react";
import FastForwardIcon from "@mui/icons-material/FastForward";
import { editor, Position } from "monaco-editor";
import {
  dbpalQuerykey,
  EscDownEvent,
  isRedisCode,
  isSQLCode,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
  QueryConn,
  QueryEvent,
} from "./Common.ts";
import { BsFillLightbulbFill } from "react-icons/bs";
import { FaUpload } from "react-icons/fa";
import { MdOutlineReply } from "react-icons/md";
import { QueryLog } from "./QueryLog.tsx";
import RefreshIcon from "@mui/icons-material/Refresh";
import {
  DataSourceKind,
  ErrAndLoc,
  getEditorText,
  MuiDataGridData,
  resizeHandlerSize,
  rustResultOk,
  secondConfirm,
  TabContext,
  shrinkAble,
  saveQueryKey,
  MonacoEditor,
  QueryIDEditorChatCtx,
  getDefaultLanguage,
  jumpToEditorText,
} from "./helper.ts";
import ArrowCircleDownRoundedIcon from "@mui/icons-material/ArrowCircleDownRounded";
import { ExportView } from "./Export.tsx";
import { PossibleParseError, TimeDuration } from "./CommonComp.tsx";
import {
  AiMarkdownModel,
  AiMarkdownModelSwitch,
  CancelButton,
  EditorBar,
  EscPressedDo,
  LoadStoreFile,
  QueryStatus,
  SaveFavoriteSqlView,
  SliderSize,
  ToggleChatShow,
} from "./CommonComp2.tsx";
import { ModifyLLMConfigModal } from "./Settings.tsx";
import { cancelTask } from "./handlers.ts";
import { Channel } from "@tauri-apps/api/core";

import {
  useBoolSwitch,
  useConfirmMsg,
  useNotification,
  useQueryExtraUI,
  useQueryLogStore,
  UseSizeConfig,
  useSizeStore,
} from "./store.tsx";
import { commands } from "./bindings.ts";

function SqlTransaction(props: { conn: QueryConn; executeTimes: number }) {
  const { conn, executeTimes } = props;
  const [commit, setCommit] = useState(true);
  const { showNotification } = useNotification();
  const [txCount, setTxCount] = useState(0);
  const tabCtx = useContext(TabContext);
  const { removeConfirmMsg, addConfirmMsg } = useConfirmMsg();

  const handleQueryCommit = useCallback(async () => {
    try {
      const c = await conn.getAutoCommit();
      setCommit(c);
    } catch (e) {
      showNotification({
        severity: "error",
        title: "get commit model failed",
        message: e,
      });
    }
  }, [conn, showNotification]);

  useEffect(() => {
    setTxCount(0);
    if (commit) {
      removeConfirmMsg(tabCtx.id);
    } else {
      addConfirmMsg(
        tabCtx.id,
        "In Transaction,better commit or rollback.uncommit transaction may lead database locks",
      );
    }
  }, [commit, tabCtx.id, removeConfirmMsg, addConfirmMsg]);
  useEffect(() => {
    return () => {
      removeConfirmMsg(tabCtx.id);
    };
  }, [removeConfirmMsg, tabCtx.id]);

  useEffect(() => {
    //这里的顺序很重要
    setTxCount((e) => e + 1);
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
      showNotification({ severity: "success", title: "commit ok" });
      handleQueryCommit();
    } catch (e) {
      showNotification({ severity: "error", title: "commit failed", message: e });
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
      showNotification({ severity: "success", title: "rollback ok" });
      handleQueryCommit();
    } catch (e) {
      showNotification({ severity: "error", title: "rollback failed", message: e });
    }
  };
  const reverseCommitMode = async () => {
    try {
      await conn.setAutoCommit(!commit);
      setCommit((e) => !e);
      showNotification({ severity: "success", title: "set commit mode ok" });
    } catch (e) {
      showNotification({ severity: "error", title: "set commit failed", message: e });
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
const tryCheckSqlSwitchKey = "try-check-sql";

function QueryButtons(props: {
  handleQuery(text: string, append: boolean);
  handleExecute(text: string);
  getText: () => string; // if is empty should yield a warning ,then I don't have to check it every time
  loading: boolean;
  disabled: boolean;
  handleCancel: () => void;
  supportExplain: boolean;
}) {
  const {
    getText,
    handleQuery,
    handleExecute,
    loading,
    disabled,
    handleCancel,
    supportExplain,
  } = props;

  return (
    <>
      <Tooltip title="query">
        <span>
          <IconButton
            loading={loading}
            onClick={() => {
              const text = getText();
              if (!text) {
                return;
              }
              handleQuery(text, false);
            }}
            disabled={disabled}
            color="primary">
            <PlayArrowIcon />
          </IconButton>
        </span>
      </Tooltip>

      <Tooltip title="execute query and append the result to tabs">
        <span>
          <IconButton
            loading={loading}
            onClick={() => {
              const text = getText();
              if (!text) {
                return;
              }
              handleQuery(text, true);
            }}
            disabled={disabled}
            color="primary">
            <FastForwardIcon />
          </IconButton>
        </span>
      </Tooltip>

      <Tooltip title="execute sql not expecting result">
        <span>
          <IconButton
            disabled={disabled}
            loading={loading}
            onClick={() => {
              const text = getText();
              if (!text) {
                return;
              }
              handleExecute(text);
            }}
            color="primary">
            <FaExclamation />
          </IconButton>
        </span>
      </Tooltip>
      {loading && (
        <CancelButton
          handleCancel={() => {
            handleCancel();
          }}
        />
      )}
      {supportExplain && (
        <Tooltip title="explain sql">
          <span>
            <IconButton
              loading={loading}
              disabled={disabled}
              onClick={() => {
                const text = getText();
                if (!text) {
                  return;
                }
                handleQuery(`EXPLAIN ${text}`, true);
              }}
              color="primary">
              <BsFillLightbulbFill />
            </IconButton>
          </span>
        </Tooltip>
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
}) {
  const { uuid, value, favoriteName, kind, extras } = props;
  const [resp, setResp] = useState<QueryRespData[]>([]);
  const [sqlErr, setSqlErr] = useState<ErrAndLoc | null>(null);
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position | null>(null);
  const [loading, setLoading] = useState(false);
  const [makeConnLoading, setMakeConnLoading] = useState(false);
  const { showNotification } = useNotification();
  const [showQueryLog, setShowQueryLog] = useState(false);
  const [fetchDur, setFetchDur] = useState(0);
  const [executeTimes, setExecuteTimes] = useState(0);
  const { queryId } = useContext(QueryIDEditorChatCtx);
  const conn = useRef<QueryConn>(new QueryConn(uuid, queryId, kind));
  const getSize = useSizeStore((s) => s.getSize);
  const [showExport, setShowExport] = useState(false);
  const [openOk, setOpenOk] = useState(false);
  const [queryStatus, setQueryStatus] = useState("");
  const tabCtx = useContext(TabContext);
  const querySomeShow = useQueryExtraUI((s) => s.extra);
  const { getSwitch, setSwitch } = useBoolSwitch();
  const tryCheckSql = getSwitch(tryCheckSqlSwitchKey);
  const { save: saveQueryLog } = useQueryLogStore();
  const editorDivDom = useRef<HTMLDivElement>(null);
  const [markdownModel, setMarkdownModel] = useState(false);

  const setEditorContext = useContext(QueryIDEditorChatCtx);
  useEffect(() => {
    if (!editor) {
      return;
    }
    setEditorContext.setEditor(editor);
  }, [editor, setEditorContext]);

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
    let x = editor.getModel().onDidChangeContent(() => {
      if (markdownModel) {
        return;
      }
      if (!tryCheckSql) {
        setSqlErr(null);
        return;
      }
      commands.checkSql(uuid, editor.getValue()).then((data) => {
        setSqlErr(data);
      });
    });
    return () => {
      x.dispose();
    };
  }, [editor, markdownModel, tryCheckSql, uuid]);

  useEffect(() => {
    setMakeConnLoading(true);
    conn.current
      .open()
      .then(() => {
        setOpenOk(true);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "establish connection failed",
          message: e,
        });
      })
      .finally(() => {
        setMakeConnLoading(false);
      });
  }, [showNotification]);

  useEffect(() => {
    // 1. Capture the current ref value into a local variable
    const currentConn = conn.current;

    return () => {
      // 2. Use the local variable, which is safely held in the closure
      if (currentConn) {
        currentConn.destroy();
      }
    };
  }, []);

  const handleReconnect = async () => {
    setMakeConnLoading(true);
    conn.current
      .open()
      .then(() => {
        showNotification({ severity: "success", title: "Reconnect OK" });
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "Reconnect Failure", message: e });
      })
      .finally(() => {
        // trigger some reload like sqltransaction
        setExecuteTimes((e) => e + 1);
        setMakeConnLoading(false);
      });
  };
  const warnDangerousSql = useCallback(async (text: string): Promise<boolean> => {
    try {
      if (await conn.current.isDangerSql(text)) {
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
  }, []);

  const handleExecute = async (text: string) => {
    const confirm = await warnDangerousSql(text);
    if (!confirm) {
      return;
    }
    setResp([]);
    setLoading(true);
    conn.current
      .executeOP(text)
      .then((e) => {
        if (getSwitch(saveQueryKey)) {
          saveQueryLog(uuid, text, rustResultOk(e));
        }
        setTimeout(() => {
          setResp([e]);
        }, 50);
        showNotification({ severity: "success", title: "execute ok" });
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "query failed", message: e });
      })
      .finally(() => {
        setExecuteTimes((e) => e + 1);
        setLoading(false);
      });
  };
  const handleQuery = useCallback(
    async (text: string, append: boolean) => {
      console.log("handle query");
      if (!append) {
        setResp([]);
      }
      const confirm = await warnDangerousSql(text);
      if (!confirm) {
        return;
      }
      setLoading(true);
      let emit = new Channel<QueryEvent>((e) => {
        // In Strict Mode, React will call your initializer function twice in order to help you find accidental impurities. This is development-only behavior and does not affect production. If your initializer function is pure (as it should be), this should not affect the behavior. The result from one of the calls will be ignored.
        setResp((resp) => {
          return appendQueryResult(resp, e, setQueryStatus);
        });
      });
      const fetchBegin = new Date();
      conn.current
        .queryOP(text, emit, getSize(UseSizeConfig.queryLimit))
        .then(() => {
          if (getSwitch(saveQueryKey)) {
            saveQueryLog(uuid, text, true);
          }
          showNotification({ severity: "success", title: "query OK" });
        })
        .catch((e) => {
          if (getSwitch(saveQueryKey)) {
            saveQueryLog(uuid, text, false);
          }
          showNotification({ severity: "error", title: "query failed", message: e });
        })
        .finally(() => {
          setExecuteTimes((e) => e + 1);
          setLoading(false);
          setFetchDur(new Date().getTime() - fetchBegin.getTime());
        });
    },
    [getSize, getSwitch, saveQueryLog, showNotification, uuid, warnDangerousSql],
  );

  useEffect(() => {
    if (editor) {
      editor.getModel()[dbpalQuerykey] = new NameAndQueryId(uuid, kind, queryId);
    }
  }, [editor, kind, queryId, uuid]);

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
    DataSourceKind.DuckDB,
  ].includes(kind);

  return (
    <div
      style={{
        height: "100%",
        overflow: "auto",
        display: "flex",
        flexDirection: "column",
        marginBottom: "300px",
      }}>
      <EscPressedDo
        doingIt={() => loading}
        setUndo={() => {
          cancelTask(queryId);
        }}
      />
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
          collapsedSize={0}>
          <QueryLog
            showQueryLog={showQueryLog}
            setShowQueryLog={setShowQueryLog}
            uuid={uuid}
            executeTimes={executeTimes}
            kind={kind}
            queryId={queryId}
          />
          <QueryStatus status={queryStatus} />
          <Stack
            direction="row"
            sx={{
              alignItems: "center",
              overflow: "auto",
              paddingBottom: "0.7rem",
            }}>
            {openOk && (
              <>
                <QueryButtons
                  disabled={markdownModel}
                  getText={() => {
                    return getEditorText(editor, showNotification);
                  }}
                  supportExplain={supportExplain}
                  handleQuery={handleQuery}
                  handleExecute={handleExecute}
                  loading={loading}
                  handleCancel={() => {
                    cancelTask(queryId);
                  }}
                />
                <Tooltip title="try check sql,right now implemented it using parser">
                  <Checkbox
                    checked={tryCheckSql}
                    onChange={(e) => {
                      setSwitch(tryCheckSqlSwitchKey, e.target.checked);
                    }}
                    disabled={markdownModel}
                    name="myCheckbox"
                    color="primary"
                  />
                </Tooltip>
              </>
            )}
            {openOk && conn.current.supportTransaction() && (
              <SqlTransaction executeTimes={executeTimes} conn={conn.current} />
            )}
            <Tooltip title={"reconnect"}>
              <span>
                <IconButton
                  loading={makeConnLoading}
                  onClick={handleReconnect}
                  color="primary">
                  <RefreshIcon />
                </IconButton>
              </span>
            </Tooltip>
            {openOk && (
              <Tooltip title="export query result to a file">
                <span>
                  <IconButton
                    loading={loading}
                    onClick={() => {
                      const value = getEditorText(editor, showNotification);
                      if (!value) {
                        return;
                      }
                      setShowExport(true);
                    }}
                    disabled={markdownModel}
                    color="primary">
                    <ArrowCircleDownRoundedIcon />
                  </IconButton>
                </span>
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
                  setShowQueryLog((prev) => !prev);
                }}
                color="primary">
                <EventNoteIcon />
              </IconButton>
            </Tooltip>

            {extras}
            <ModifyLLMConfigModal />
            <SliderSize config={UseSizeConfig.queryLimit} />
            <TimeDuration dur={fetchDur} />
            <ToggleChatShow />
            <AiMarkdownModelSwitch
              setMarkdownModel={setMarkdownModel}
              markdownModel={markdownModel}
            />
            {querySomeShow.get(tabCtx.id)}
          </Stack>
          <Divider />

          <AiMarkdownModel
            markdownModel={markdownModel}
            editor={editor}
            appCodeHandleEdit={({ code }) => {
              setMarkdownModel(false);
              jumpToEditorText(editor, code);
            }}
            appcodeExtra={({ code, language }) => {
              if (isSQLCode(code, language) || isRedisCode(code)) {
                return (
                  <QueryButtons
                    disabled={false}
                    getText={() => {
                      return code;
                    }}
                    supportExplain={supportExplain}
                    loading={loading}
                    handleQuery={handleQuery}
                    handleExecute={handleExecute}
                    handleCancel={() => {
                      cancelTask(queryId);
                    }}
                  />
                );
              } else {
                return <> </>;
              }
            }}
          />
          <div
            style={{
              flex: 1,
              minHeight: 0,
              height: "100px",
              display: markdownModel ? "none" : "block",
            }}
            ref={editorDivDom}>
            <MonacoEditor
              language={markdownModel ? "markdown" : getDefaultLanguage(kind)}
              value={value}
              onMount={handleEditorDidMount}
            />
          </div>
          {!markdownModel && (
            <EditorBar
              left={
                <Stack
                  style={{
                    flex: 1,
                  }}
                  direction="row">
                  {sqlErr && <PossibleParseError parseErr={sqlErr} editor={editor} />}
                </Stack>
              }
              pos={pos}
            />
          )}
        </Panel>

        {resp.length > 0 && (
          <>
            <PanelResizeHandle
              style={{ height: resizeHandlerSize }}
              className="resize-handle"
            />
            <Panel
              style={{ overflow: "auto" }}
              collapsible
              collapsedSize={0}
              maxSize={100}
              minSize={10}>
              <div style={{ height: "100%" }}>
                <MultiQueryResp data={resp} />
              </div>
            </Panel>
          </>
        )}
      </PanelGroup>

      {showExport && (
        <Modal
          open={showExport}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowExport(false);
          })}>
          <Box sx={{ ...modalStyle, width: "45%" }}>
            <ExportView
              supportKind={["csv", "json", "jsonline", "xlsx", "html", "markdown"]}
              exportOP={async (kind, file, taskId, sizeEmitter) => {
                const sql = getEditorText(editor, showNotification);
                return conn.current.exportQueryResult(
                  queryId,
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
  setQueryStatus: (string) => void,
): QueryRespData[] {
  switch (event.kind) {
    case "structure":
      return [...data, { Ok: event.resp }];
    case "err":
      return [...data, { Err: event.err }];
    case "data": {
      if (event.data.length == 0) {
        return data;
      }
      let data0 = event.data[0];
      setQueryStatus(
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
        setQueryStatus(
          "should be able to find the index the place data ,this is internal error",
        );
        return data;
      }
      let cop = { ...data[index].Ok } as MuiDataGridData;
      cop.data = [
        ...cop.data,
        ...event.data.map((d) => {
          let obj = {};
          d.forEach((e, index) => {
            obj[cop.fields[index].field] = e;
          });
          return obj;
        }),
      ];
      let arr = [...data];
      arr[index].Ok = cop;
      return arr;
    }
    case "status": {
      setQueryStatus(event.status);
      return data;
    }
    case "end":
      setQueryStatus("");
      return data;
    default:
      console.error("backend give some data which we can not handle", event);
  }
}

export { SqlQuery };
