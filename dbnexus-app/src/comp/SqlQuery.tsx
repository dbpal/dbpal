import { MultiQueryResp } from "./QueryResp";
import { Chip, Divider, IconButton, Stack, Tooltip } from "@mui/material";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import PlayArrowIcon from "@mui/icons-material/PlayArrow";
import { FaExclamation } from "react-icons/fa";
import { ReactNode, useCallback, useContext, useEffect, useRef, useState } from "react";
import FastForwardIcon from "@mui/icons-material/FastForward";
import { editor } from "monaco-editor";
import { isRedisCode, isSQLCode, NameAndQueryId, QueryConn } from "./Common.ts";
import { BsFillLightbulbFill } from "react-icons/bs";
import { FaUpload } from "react-icons/fa";
import { MdOutlineReply } from "react-icons/md";
import RefreshIcon from "@mui/icons-material/Refresh";
import {
  DataSourceKind,
  getEditorText,
  resizeHandlerSize,
  rustResultOk,
  secondConfirm,
  TabContext,
  shrinkAble,
  saveQueryKey,
  MonacoEditor,
  QueryIDEditorChatCtx,
  getDatabaseKindLanguage,
  jumpToEditorText,
  QueryRespData,
  DbnameContext,
} from "./helper.ts";
import ArrowCircleDownRoundedIcon from "@mui/icons-material/ArrowCircleDownRounded";
import { ExportView } from "./Export.tsx";
import {
  AiMarkdownModel,
  AiMarkdownModelSwitch,
  CancelButton,
  EscPressedDo,
  FetchTimeAndLoadingStatus,
  LoadStoreFile,
  QueryStatus,
  SaveFavoriteSqlView,
  ShowQueryLog,
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
  useOneShotDialogUI,
  useQueryExtraUI,
  useQueryLogStore,
  UseSizeConfig,
  useSizeStore,
} from "./store.tsx";
import { QueryEvent } from "./bindings.ts";

function SqlTransaction(props: { conn: QueryConn; executeTimes: number }) {
  const { conn, executeTimes } = props;
  const [commit, setCommit] = useState(true);
  const { showNotification } = useNotification();
  const [txCount, setTxCount] = useState(0);
  const tabCtx = useContext(TabContext);
  const { removeConfirmMsg, addConfirmMsg } = useConfirmMsg();

  const queryCommit = useCallback(async () => {
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

    return () => {
      removeConfirmMsg(tabCtx.id);
    };
  }, [commit, tabCtx.id, removeConfirmMsg, addConfirmMsg]);

  useEffect(() => {
    //这里的顺序很重要
    setTxCount((e) => e + 1);
    queryCommit();
  }, [executeTimes, queryCommit]);

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
      queryCommit();
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
      queryCommit();
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

function QueryButtons(props: {
  handleQuery: (text: string, append: boolean) => void;
  handleExecute: (text: string) => void;
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
      {loading && (
        <CancelButton
          handleCancel={() => {
            handleCancel();
          }}
        />
      )}
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
  value?: string;
  favoriteName?: string;
  extraButtons?: ReactNode;
}) {
  const { dbName, kind } = useContext(DbnameContext);
  const { value, favoriteName, extraButtons } = props;
  const [resp, setResp] = useState<QueryRespData[]>([]);
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [loading, setLoading] = useState(false);
  const [makeConnLoading, setMakeConnLoading] = useState(false);
  const { showNotification } = useNotification();
  const [executeTimes, setExecuteTimes] = useState(0);
  const { queryId } = useContext(QueryIDEditorChatCtx);
  const conn = useRef<QueryConn>(new QueryConn(dbName, queryId, kind));
  const getSize = useSizeStore((s) => s.getSize);
  const [openOk, setOpenOk] = useState(false);
  const [queryStatus, setQueryStatus] = useState("");
  const tabCtx = useContext(TabContext);
  const querySomeShow = useQueryExtraUI((s) => s.extra);
  const { save: saveQueryLog } = useQueryLogStore();
  const [markdownModel, setMarkdownModel] = useState(false);
  const { setOneShotUI: setOneShotUI } = useOneShotDialogUI();
  const { getSwitch } = useBoolSwitch();
  const setEditorContext = useContext(QueryIDEditorChatCtx);
  useEffect(() => {
    if (!editor) {
      return;
    }
    setEditorContext.setEditor(editor);
  }, [editor, setEditorContext]);

  const handleEditorDidMount = useCallback((ed: editor.IStandaloneCodeEditor) => {
    setEditor(ed);
  }, []);

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
        setOpenOk(true);
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
    setLoading(true);
    conn.current
      .executeOP(text)
      .then((e) => {
        if (getSwitch(saveQueryKey)) {
          saveQueryLog(dbName, text, rustResultOk(e));
        }
        setTimeout(() => {
          setResp([...resp, e]);
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
      conn.current
        .queryOP(text, emit, getSize(UseSizeConfig.queryLimit))
        .then(() => {
          if (getSwitch(saveQueryKey)) {
            saveQueryLog(dbName, text, true);
          }
          showNotification({ severity: "success", title: "query OK" });
        })
        .catch((e) => {
          if (getSwitch(saveQueryKey)) {
            saveQueryLog(dbName, text, false);
          }
          showNotification({ severity: "error", title: "query failed", message: e });
        })
        .finally(() => {
          setExecuteTimes((e) => e + 1);
          setLoading(false);
        });
    },
    [getSize, getSwitch, saveQueryLog, showNotification, dbName, warnDangerousSql],
  );

  useEffect(() => {
    if (editor) {
      editor.getModel()._dbpal_query_id = new NameAndQueryId(dbName, kind, queryId);
    }
  }, [editor, kind, queryId, dbName]);

  const supportExplain = [
    DataSourceKind.Sqlite,
    DataSourceKind.LibSql,
    DataSourceKind.MariaDB,
    DataSourceKind.TiDB,
    DataSourceKind.Mysql,
    DataSourceKind.Oracle,
    DataSourceKind.SqlServer,
    DataSourceKind.Postgres,
    DataSourceKind.Cockroach,
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
                    return getEditorText(editor, showNotification, true);
                  }}
                  supportExplain={supportExplain}
                  handleQuery={handleQuery}
                  handleExecute={handleExecute}
                  loading={loading}
                  handleCancel={() => {
                    cancelTask(queryId);
                  }}
                />
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
                      const value = getEditorText(editor, showNotification, true);
                      if (!value) {
                        return;
                      }
                      setOneShotUI(
                        <ExportView
                          supportKind={[
                            "csv",
                            "json",
                            "jsonline",
                            "xlsx",
                            "html",
                            "markdown",
                          ]}
                          exportOP={async (kind, file, taskId, sizeEmitter) => {
                            const sql = getEditorText(editor, showNotification, true);
                            return conn.current.exportQueryResult(
                              queryId,
                              sql,
                              kind,
                              file,
                              taskId,
                              sizeEmitter,
                            );
                          }}
                          name="export_query_result"
                        />,
                        `export query result to a file`,
                      );
                    }}
                    disabled={markdownModel}
                    color="primary">
                    <ArrowCircleDownRoundedIcon />
                  </IconButton>
                </span>
              </Tooltip>
            )}
            <SaveFavoriteSqlView favoriteName={favoriteName} editor={editor} />
            {editor && (
              <LoadStoreFile
                getQuery={() => {
                  return editor.getValue();
                }}
                handleLoadFile={(content) => {
                  editor.setValue(content);
                }}
                kind={kind}
              />
            )}
            <ShowQueryLog />

            {extraButtons}
            <ModifyLLMConfigModal />

            <ToggleChatShow />
            <AiMarkdownModelSwitch
              setMarkdownModel={setMarkdownModel}
              markdownModel={markdownModel}
              dbName={dbName}
            />
            {querySomeShow.get(tabCtx.id)}

            <FetchTimeAndLoadingStatus isLoading={loading} />
            <SliderSize config={UseSizeConfig.queryLimit} />
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
            }}>
            <MonacoEditor
              language={markdownModel ? "markdown" : getDatabaseKindLanguage(kind)}
              value={value}
              onMount={handleEditorDidMount}
            />
          </div>
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
    </div>
  );
}

function appendQueryResult(
  data: QueryRespData[],
  event: QueryEvent,
  setQueryStatus: (_: string) => void,
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
      const index = data.findLastIndex((x) => x.Ok);
      if (index === -1) {
        setQueryStatus(
          "should be able to find the index the place data ,this is internal error",
        );
        return data;
      }
      let cop = { ...data[index].Ok };
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
