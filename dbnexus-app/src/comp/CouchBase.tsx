import {
  Autocomplete,
  Box,
  Button,
  ButtonGroup,
  Chip,
  Divider,
  IconButton,
  InputAdornment,
  Modal,
  Pagination,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import ArrowCircleDownRoundedIcon from "@mui/icons-material/ArrowCircleDownRounded";
import EventNoteIcon from "@mui/icons-material/EventNote";
import {
  LoadStoreFile,
  QueryStatus,
  SetFilterSort,
  SimpleFlexEditor,
  SliderSize,
  TableAddButton,
  ToggleChatShow,
} from "./CommonComp2.tsx";
import { ReactNode, useCallback, useContext, useEffect, useRef, useState } from "react";
import {
  DataSourceKind,
  ErrAndLoc,
  generateRandomString,
  getEditorText,
  handleCopyToClipboard,
  MuiDataGridData,
  QueryLogHandler,
  resizeHandlerSize,
  saveAndSetSize,
  SavePageSize,
  secondConfirm,
  SettingContext,
  shrinkAble,
  validJson,
} from "./helper.ts";
import ReactJsonView, { InteractionProps, ThemeKeys } from "@microlink/react-json-view";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import PlayArrowIcon from "@mui/icons-material/PlayArrow";
import FilterAltIcon from "@mui/icons-material/FilterAlt";
import { FaArrowsAltV } from "react-icons/fa";
import { ExecutingTime, ModalHeader, SelectPageSize } from "./CommonComp.tsx";
import { Channel, invoke } from "@tauri-apps/api/core";
import types from "./ApiStruct.ts";
import {
  EscDownEvent,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
  QueryEvent,
  SetAutoCompletionName,
} from "./Common.ts";
import { editor, Position } from "monaco-editor";
import ClearIcon from "@mui/icons-material/Clear";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { QueryLog } from "./QueryLog.tsx";
import { ExportView } from "./Export.tsx";
import { QueryResp } from "./QueryResp.tsx";
import { useNotification } from "./store.ts";

type Data = {
  data: { data: string; id: string }[];
  count: number;
};

function newDataDefault(): Data {
  return { count: 0, data: [] };
}

enum TableViewMode {
  Editor = "editor",
  Json = "json",
}

class TableViewModeSaver {
  static key = "couchbase-table-view-mode";
  static load(): TableViewMode {
    return (
      (localStorage.getItem(TableViewModeSaver.key) as TableViewMode) ||
      TableViewMode.Editor
    );
  }

  static save(x: TableViewMode) {
    localStorage.setItem(TableViewModeSaver.key, x);
  }
}

class JsonEditorThemeSaver {
  static key = "couchbase-json-editor-theme";
  static load(): ThemeKeys {
    return (
      (localStorage.getItem(JsonEditorThemeSaver.key) as ThemeKeys) || "rjv-default"
    );
  }
  static save(x: ThemeKeys) {
    localStorage.setItem(JsonEditorThemeSaver.key, x);
  }
}

function ViewModeSwitcher({ setViewMode, viewMode }) {
  return (
    <ButtonGroup>
      {[TableViewMode.Editor, TableViewMode.Json].map((m) => (
        <Button
          onClick={() => {
            setViewMode(m);
          }}
          key={m}
          disabled={viewMode == m}
        >
          {m}
        </Button>
      ))}
    </ButtonGroup>
  );
}

function CouchBaseTable(props: { uuid: string; table: string }) {
  const { uuid, table } = props;
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(SavePageSize.couchbase.get());
  const [where, setWhere] = useState("");
  const [sort, setSort] = useState("");
  const [showSetSort, setShowSetSort] = useState(false);
  const [showSetWhere, setShowSetWhere] = useState(false);
  const kind = DataSourceKind.CouchBase;
  const [resp, setResp] = useState<Data>(newDataDefault());
  const [gridColCount, setGridColCount] = useState(3);
  const [editorHeight, setEditorHeight] = useState(500);
  const [loading, setLoading] = useState(true);
  const [queryStatus, setQueryStatus] = useState("");
  const { showNotification } = useNotification((s) => s);
  const [showAdd, setShowAdd] = useState(false);
  const [addModalBox, setAddModalBox] = useState(null);
  const [viewMode, setViewMode] = useState(TableViewModeSaver.load());
  useEffect(() => {
    TableViewModeSaver.save(viewMode);
  }, [viewMode]);
  const [jsonEditorTheme, setJsonEditorTheme] = useState(JsonEditorThemeSaver.load());
  useEffect(() => {
    JsonEditorThemeSaver.save(jsonEditorTheme);
  }, [jsonEditorTheme]);

  useEffect(() => {
    SavePageSize.couchbase.save(pageSize);
  }, [pageSize]);

  useEffect(() => {
    if (addModalBox) {
      const clean = saveAndSetSize(addModalBox, "couchbase-table-insert");
      return () => {
        clean();
      };
    }
  }, [addModalBox]);

  const handleReload = useCallback(() => {
    const c = new Channel<QueryEvent>((event) => {
      switch (event.kind) {
        case "structure": {
          setResp((resp) => {
            return { ...resp, count: event.resp.total };
          });
          break;
        }
        case "data": {
          const news = event.data.map((x) => {
            return { data: x[1], id: x[0] };
          });
          setResp((resp) => {
            return { ...resp, data: [...resp.data, ...news] };
          });
          setQueryStatus(`received ${news.length} rows`);
          break;
        }
        case "status": {
          setQueryStatus(event.status);
          break;
        }
        case "err": {
          showNotification("error", "load data failed", event.err);
          break;
        }
        case "end": {
          setQueryStatus("");
          break;
        }
      }
    });
    setResp(newDataDefault());
    setLoading(true);
    invoke("show_table", {
      name: uuid,
      tableName: table,
      isView: false,
      para: {
        where_str: where ? where : undefined,
        sort_str: sort ? sort : undefined,
        page_size: pageSize,
        page: page,
      } as types.ShowTableParameter,
      emit: c,
    })
      .catch((e) => {
        showNotification("error", "load data failed", e);
      })
      .finally(() => {
        setLoading(false);
      });
  }, [page, pageSize, showNotification, sort, table, uuid, where]);
  useEffect(() => {
    handleReload();
  }, [handleReload]);
  return (
    <Stack style={{ height: "100%" }}>
      <QueryStatus status={queryStatus} />
      <Stack
        direction="row"
        style={{ alignItems: "center", justifyContent: "space-between" }}
      >
        <Stack direction="row" style={{ alignItems: "center" }}>
          <Button
            onClick={() => {
              handleReload();
            }}
            loading={loading}
          >
            reload
          </Button>
          <ViewModeSwitcher viewMode={viewMode} setViewMode={setViewMode} />
          <Tooltip title="filter something like 'age=18'">
            <Chip
              label={where}
              onClick={() => {
                setShowSetWhere(true);
              }}
              style={shrinkAble}
              size="medium"
              color="primary"
              icon={<FilterAltIcon />}
            />
          </Tooltip>
          <Tooltip title="sort something like 'age asc'">
            <Chip
              label={sort}
              onClick={() => {
                setShowSetSort(true);
              }}
              style={shrinkAble}
              size="medium"
              color="secondary"
              variant="outlined"
              icon={<FaArrowsAltV />}
            />
          </Tooltip>
          <TableAddButton
            handleAdd={() => {
              setShowAdd(true);
            }}
          />
        </Stack>
        <Typography variant="h5"> {table} </Typography>
        <Stack direction="row">
          <SliderSize
            min={1}
            max={5}
            notice="column count"
            step={1}
            handleValue={(v) => {
              setGridColCount(v);
            }}
            storeKey="couchbase-table-view-column-count"
            name="column"
          />
          {viewMode == TableViewMode.Editor && (
            <SliderSize
              name="height"
              min={100}
              max={1000}
              notice="mongodb editor height"
              step={10}
              handleValue={(v) => {
                setEditorHeight(v);
              }}
              storeKey="couchbase-table-editor-height"
            />
          )}
        </Stack>
      </Stack>
      <div
        style={{
          flexGrow: 1,
          flexShrink: 1,
          flexBasis: "0",
          height: "100px",
          overflow: "auto",
          display: "grid",
          gridTemplateColumns: `repeat(${gridColCount}, 1fr)`,
          paddingRight: "1.2rem",
        }}
      >
        {resp.data.map((item) => (
          <div
            style={{
              height: viewMode == TableViewMode.Editor && editorHeight + "px",
              position: "relative",
            }}
            key={item.id}
          >
            <CouchBaseEditor
              docid={item.id}
              jsonTheme={jsonEditorTheme ? jsonEditorTheme : ""}
              viewMode={viewMode}
              handleValueButton="modify"
              value={item.data}
              handleDelete={async () => {
                await invoke("table_delete", {
                  name: uuid,
                  tableName: table,
                  wheres: { id: item.id },
                });
                setResp((resp) => {
                  return {
                    count: resp.count - 1,
                    data: resp.data.filter((y) => y.id != item.id),
                  };
                });
              }}
              warnModify
              handleValue={async (newV) => {
                await invoke("table_update", {
                  name: uuid,
                  tableName: table,
                  values: {
                    id: item.id,
                    value: newV,
                  },
                  wheres: {},
                });
                setResp((resp) => {
                  return {
                    count: resp.count,
                    data: resp.data.map((e) =>
                      e.id == item.id ? { ...e, data: newV } : e,
                    ),
                  } as Data;
                });
              }}
            />
          </div>
        ))}
      </div>
      <Stack
        direction="row"
        style={{ alignItems: "center", justifyContent: "space-between" }}
      >
        <Stack direction="row" style={{ alignItems: "center" }}>
          {viewMode == TableViewMode.Json && (
            <SetTheme theme={jsonEditorTheme} setTheme={setJsonEditorTheme} />
          )}

          <Typography color="info">
            result {resp.data.length} of {resp.count}
          </Typography>
        </Stack>

        <Stack direction="row" style={{ alignItems: "center" }}>
          <SelectPageSize
            pageSize={pageSize}
            handleChange={(s) => {
              setPageSize(s);
            }}
            options={[2, 3, 10].map((x) => x * gridColCount)}
          />

          <Pagination
            page={page}
            onChange={(_, p) => {
              setPage(p);
            }}
            defaultPage={1}
            count={Math.ceil(resp.count / pageSize)}
          />
        </Stack>
      </Stack>
      {showSetWhere && (
        <SetFilterSort
          tableName={table}
          open={showSetWhere}
          value={where}
          setValue={(v) => {
            setWhere(v);
          }}
          handleClose={() => {
            setShowSetWhere(false);
          }}
          label="filter"
          kind={kind}
          uuid={uuid}
        />
      )}
      {showSetSort && (
        <SetFilterSort
          tableName={table}
          open={showSetSort}
          value={sort}
          setValue={(v) => {
            setSort(v);
          }}
          kind={kind}
          handleClose={() => {
            setShowSetSort(false);
          }}
          label="sort"
          uuid={uuid}
        />
      )}

      {showAdd && (
        <Modal
          open={showAdd}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowAdd(false);
          })}
        >
          <Box
            sx={{ resize: "both", ...modalStyle }}
            ref={setAddModalBox}
            width="80%"
            height="30vh"
          >
            <ModalHeader
              title="insert data"
              handleClose={() => {
                setShowAdd(false);
              }}
            />
            <CouchBaseInsert uuid={uuid} table={table} />
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function CouchBaseEditor(props: {
  value: string;
  handleValue: (v: string) => Promise<void>;
  handleDelete?: () => Promise<void>;
  handleValueButton: string;
  barExtra?: ReactNode;
  warnModify: boolean;
  viewMode: TableViewMode;
  jsonTheme: string;
  docid?: string;
}) {
  const {
    value,
    handleValue,
    handleDelete,
    handleValueButton,
    barExtra,
    warnModify,
    viewMode,
    jsonTheme,
    docid,
  } = props;
  const setting = useContext(SettingContext);
  const [jsonErr, setJsonErr] = useState<ErrAndLoc | null>(null);
  const { showNotification } = useNotification((s) => s);
  const [jsonErrPos, setJsonErrPos] = useState<Position>(null);
  const [newValue, setNewValue] = useState(value);
  const [loading, setLoading] = useState(false);
  const [changed, setChanged] = useState(false);
  const doModify = async () => {
    if (warnModify) {
      const confirm = await secondConfirm({ title: "sure to modify?" });
      if (!confirm) {
        return;
      }
    }
    setLoading(true);
    handleValue(newValue)
      .then(() => {
        showNotification("success", `${handleValueButton} OK`);
        setChanged(false);
      })
      .catch((e) => {
        showNotification("error", `${handleValueButton} failure`, e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  const doDelete = handleDelete
    ? async () => {
        const confirm = await secondConfirm({ title: "sure to delete?" });
        if (!confirm) {
          return;
        }
        setLoading(true);
        handleDelete()
          .then(() => {
            showNotification("success", "delete OK");
          })
          .catch((e) => {
            showNotification("error", "delete failure", e);
          })
          .finally(() => {
            setLoading(false);
          });
      }
    : undefined;

  const stringify = (v: InteractionProps) => {
    // TODO number auto convert to string
    // how to fix this.
    return JSON.stringify(v.updated_src, null, setting.editor.tabSize);
  };
  return (
    <Stack style={{ height: "100%", flex: 1 }}>
      {viewMode == TableViewMode.Editor && (
        <SimpleFlexEditor
          value={newValue}
          handleValue={async (newValue) => {
            try {
              const x = await validJson(newValue);
              setJsonErr(x);
            } catch (e) {
              showNotification("error", "call valid json failed", e);
              return;
            }
            setNewValue(newValue);
            setChanged(true);
          }}
          language="json"
          errPos={jsonErrPos}
          barLeft={
            <Stack direction="row">
              {jsonErr && (
                <Typography
                  color="error"
                  style={{ cursor: "pointer" }}
                  onClick={() => {
                    if (!jsonErr) {
                      return;
                    }
                    setJsonErrPos(new Position(jsonErr.line, jsonErr.column));
                  }}
                >
                  {jsonErr.msg}
                </Typography>
              )}
            </Stack>
          }
        />
      )}
      {viewMode == TableViewMode.Json && (
        <ReactJsonView
          onEdit={
            doModify
              ? (x) => {
                  setNewValue(stringify(x));
                  setChanged(true);
                }
              : undefined
          }
          onAdd={
            doModify
              ? (x) => {
                  setNewValue(stringify(x));
                  setChanged(true);
                }
              : undefined
          }
          onDelete={
            doModify
              ? (x) => {
                  setNewValue(stringify(x));
                  setChanged(true);
                }
              : undefined
          }
          theme={jsonTheme as ThemeKeys}
          src={JSON.parse(newValue)}
          style={{ minHeight: "11rem" }}
        />
      )}

      <Stack direction="row" style={{ alignItems: "center" }}>
        {doModify && !jsonErr && (
          <>
            <Button
              onClick={() => {
                doModify();
              }}
              loading={loading}
            >
              {handleValueButton}
            </Button>
          </>
        )}
        {doDelete && (
          <Button
            onClick={() => {
              doDelete();
            }}
            color="warning"
          >
            <DeleteForeverIcon />
          </Button>
        )}
        {barExtra}
        {docid && (
          <Tooltip title="copy id">
            <Typography
              color="info"
              style={{ cursor: "pointer", alignContent: "center" }}
              onClick={() => {
                handleCopyToClipboard(docid, showNotification);
              }}
            >
              #{docid}
            </Typography>
          </Tooltip>
        )}
        {changed && <Typography color="warning"> modified </Typography>}
      </Stack>
    </Stack>
  );
}

function CouchBaseInsert({ uuid, table }) {
  const [id, setId] = useState("");
  const [viewMode, setViewMode] = useState(TableViewModeSaver.load());
  return (
    <CouchBaseEditor
      jsonTheme={JsonEditorThemeSaver.load()}
      viewMode={viewMode}
      warnModify={false}
      // deno-lint-ignore jsx-curly-braces
      value={"{\n}"}
      barExtra=<>
        <Button
          onClick={() => {
            setId(generateRandomString());
          }}
          variant="text"
        >
          gen
        </Button>
        <TextField
          label="id"
          value={id}
          onChange={(e) => {
            setId(e.target.value);
          }}
          slotProps={{
            input: {
              endAdornment: id && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setId("");
                    }}
                    edge="end"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
          required
        />
        <ViewModeSwitcher viewMode={viewMode} setViewMode={setViewMode} />
      </>
      handleValueButton="insert"
      handleValue={(value) => {
        if (!id.trim()) {
          throw "id is empty";
        }
        return invoke("table_insert", {
          name: uuid,
          tableName: table,
          values: {
            id: id.trim(),
            value,
          },
        });
      }}
    />
  );
}

enum QueryViewMode {
  Table = "table",
  Json = "json",
}

class QueryViewModeSaver {
  static key = "couchbase-query-view-mode";
  static load(): QueryViewMode {
    return (
      (localStorage.getItem(QueryViewModeSaver.key) as QueryViewMode) ||
      QueryViewMode.Json
    );
  }

  static save(x: QueryViewMode) {
    localStorage.setItem(QueryViewModeSaver.key, x);
  }
}

function CouchBaseQuery(props: {
  uuid: string;
  value?: string;
  handleEditor: (ed: editor.IStandaloneCodeEditor) => void;
}) {
  const { uuid, value, handleEditor } = props;
  const [resp, setResp] = useState<object[]>([]);
  const [loading, setLoading] = useState(false);
  const [gridColCount, setGridColCount] = useState(3);
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const maxRow = useRef(2000);
  const [queryStatus, setQueryStatus] = useState("");
  const { showNotification } = useNotification((s) => s);
  const [showQueryLog, setShowQueryLog] = useState(false);
  const kind = DataSourceKind.CouchBase;
  const [executeTimes, setExecuteTimes] = useState(0);
  const [showExport, setShowExport] = useState(false);
  const [viewMode, setViewMode] = useState(QueryViewModeSaver.load());
  const [tableResp, setTableResp] = useState<MuiDataGridData>(null);

  useEffect(() => {
    QueryViewModeSaver.save(viewMode);
  }, [viewMode]);

  useEffect(() => {
    if (viewMode != QueryViewMode.Table) {
      return;
    }
    if (resp.length == 0) {
      return;
    }
    const fields: types.Field[] = [];
    const data: object[] = [];

    function encodeValue(x) {
      switch (typeof x) {
        case "object":
          return JSON.stringify(x);
        default:
          return String(x);
      }
    }
    for (const row of resp) {
      let obj = row;
      {
        const keys = Object.keys(obj);
        if (keys.length == 1 && typeof obj[keys[0]] == "object") {
          obj = obj[Object.keys(obj)[0]];
        }
      }
      const d = {};
      for (const key in obj) {
        if (typeof obj[key] == "object" && !Array.isArray(obj[key])) {
          for (const k in obj[key]) {
            const fKey = `${key}.${k}`;

            if (-1 == fields.findIndex((x) => x.field == fKey)) {
              fields.push({
                field: fKey,
              } as types.Field);
            }
            d[fKey] = encodeValue(obj[key][k]);
          }
        } else {
          d[key] = encodeValue(obj[key]);
          if (-1 == fields.findIndex((x) => x.field == key)) {
            fields.push({
              field: key,
            } as types.Field);
          }
        }
      }
      data.push(d);
    }
    setTableResp({ fields, data, sql: "", total: null });
  }, [resp, uuid, viewMode]);

  const [jsonEditorTheme, setJsonEditorTheme] = useState<ThemeKeys>(
    JsonEditorThemeSaver.load(),
  );
  useEffect(() => {
    JsonEditorThemeSaver.save(jsonEditorTheme);
  }, [jsonEditorTheme]);
  const handleQuery = () => {
    const text = getEditorText(editor, showNotification);
    if (!text) {
      return;
    }
    setLoading(true);
    setResp([]);
    const channel = new Channel<QueryEvent>((event) => {
      switch (event.kind) {
        case "structure": {
          console.error("server should not emit this.", event);
          break;
        }
        case "data": {
          const xs = event.data.map((x) => JSON.parse(x[0]));
          setResp((resp) => {
            return [...resp, ...xs];
          });
          setQueryStatus(`receive ${xs.length} rows`);
          break;
        }
        case "err":
          showNotification("error", "query failed", event.err);
          break;
        case "end":
          setQueryStatus("");
          break;
        case "status":
          setQueryStatus(event.status);
          break;
      }
    });
    invoke("query_sql", {
      name: uuid,
      queryId: "",
      maxRow: maxRow.current,
      sql: text,
      emit: channel,
    })
      .catch((e) => {
        showNotification("error", "query failed", e);
        new QueryLogHandler(uuid).save(text, false);
      })
      .then(() => {
        new QueryLogHandler(uuid).save(text, true);
      })
      .finally(() => {
        setLoading(false);
        setExecuteTimes((e) => e + 1);
      });
  };
  return (
    <PanelGroup
      style={{ height: "100%" }}
      direction="vertical"
      autoSaveId="couchbase-query"
    >
      <Panel
        collapsible
        collapsedSize={0}
        minSize={10}
        defaultSize={35}
        style={{ display: "flex", flexDirection: "column" }}
      >
        <QueryLog
          showQueryLog={showQueryLog}
          setShowQueryLog={setShowQueryLog}
          uuid={uuid}
          executeTimes={executeTimes}
          kind={kind}
        />
        <QueryStatus status={queryStatus} />
        <Stack
          direction="row"
          style={{
            alignItems: "center",
            justifyContent: "space-between",
            paddingBottom: "0.5rem",
            overflow: "auto",
          }}
        >
          <Stack direction="row" style={{ alignItems: "center" }}>
            <Tooltip title="query">
              <IconButton
                loading={loading}
                onClick={() => {
                  handleQuery();
                }}
                color="primary"
              >
                <PlayArrowIcon />
              </IconButton>
            </Tooltip>
            <LoadStoreFile
              getQuery={() => {
                return editor.getValue();
              }}
              handleLoadFile={(_, c) => {
                editor.setValue(c);
              }}
              kind={DataSourceKind.CouchBase}
            />

            <ButtonGroup>
              {[QueryViewMode.Json, QueryViewMode.Table].map((e) => (
                <Button
                  key={e}
                  disabled={e == viewMode}
                  onClick={() => {
                    setViewMode(e);
                  }}
                >
                  {e}
                </Button>
              ))}
            </ButtonGroup>
          </Stack>
          <Stack direction="row" style={{ alignItems: "center" }}>
            <Typography color="info"> rows:{resp.length} </Typography>
            <Tooltip title="export query result to a file">
              <IconButton
                loading={loading}
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
            <Tooltip title="query log">
              <IconButton
                onClick={() => {
                  setShowQueryLog((prev) => !prev);
                }}
                color="primary"
              >
                <EventNoteIcon />
              </IconButton>
            </Tooltip>

            {viewMode == QueryViewMode.Json && (
              <SliderSize
                min={1}
                max={5}
                notice="column count"
                step={1}
                handleValue={(v) => {
                  setGridColCount(v);
                }}
                storeKey="couchbase-query-column-size"
                name="column"
              />
            )}
            {viewMode == QueryViewMode.Json && (
              <SetTheme theme={jsonEditorTheme} setTheme={setJsonEditorTheme} />
            )}

            <ToggleChatShow do_query={handleQuery} />

            <SliderSize
              storeKey="couchbase-sql-query"
              min={1000}
              max={100000}
              step={1000}
              handleValue={(v) => {
                maxRow.current = v;
              }}
              notice={
                <Typography>
                  limit max rows can be query.we don&apos;t want OOM!!!😀
                  <br />
                  dbpal use Stream technology to query data,so we can query large data
                </Typography>
              }
            />
            <ExecutingTime loading={loading} />
          </Stack>
        </Stack>
        <SimpleFlexEditor
          value={value}
          language="sql"
          handleEditor={(e) => {
            setEditor(e);
            handleEditor(e);
            SetAutoCompletionName.set(
              e.getModel().id,
              new NameAndQueryId(uuid, DataSourceKind.CouchBase),
            );
          }}
        />
      </Panel>
      <PanelResizeHandle
        style={{
          height: resizeHandlerSize,
        }}
        id="chat-resize-bar"
        className="resize-handle"
      />

      <Panel collapsible collapsedSize={0} minSize={10} style={{ overflow: "auto" }}>
        {viewMode == QueryViewMode.Json && (
          <div
            style={{
              overflow: "auto",
              display: "grid",
              gridTemplateColumns: `repeat(${gridColCount}, 1fr)`,
              paddingRight: "1.2rem",
            }}
          >
            {resp.map((obj, index) => {
              return (
                <div key={index}>
                  <ReactJsonView
                    src={obj}
                    key={index}
                    theme={jsonEditorTheme as ThemeKeys}
                  />
                  <Divider />
                </div>
              );
            })}
          </div>
        )}
        {viewMode == QueryViewMode.Table && tableResp && <QueryResp data={tableResp} />}
      </Panel>
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
              supportKind={["json", "jsonline"]}
              exportOP={(kind, file, taskId, sizeEmitter) => {
                const sql = getEditorText(editor, showNotification);
                return invoke<void>("export_query_result_to_file", {
                  name: uuid,
                  taskId,
                  file,
                  kind,
                  sizeEmitter,
                  sql,
                  queryId: "",
                });
              }}
              handleClose={() => {
                setShowExport(false);
              }}
              name="export_query_result"
            />
          </Box>
        </Modal>
      )}
    </PanelGroup>
  );
}

// 帮我 https://github.com/microlinkhq/react-json-view 内置主题整理成js数组

function SetTheme({ theme, setTheme }) {
  return (
    <Autocomplete
      options={themes}
      style={{ width: "16rem" }}
      value={theme}
      renderInput={(params) => <TextField {...params} label={`json editor theme`} />}
      freeSolo={false}
      onChange={(_, v) => {
        if (v) {
          setTheme(v);
        }
      }}
    />
  );
}

const themes = [
  "rjv-default",
  "apathy",
  "ashes",
  "atelierDune",
  "atelierForest",
  "atelierHeath",
  "atelierLakeside",
  "atelierSeaside",
  "bespin",
  "brewer",
  "bright",
  "chalk",
  "codeschool",
  "colors",
  "eighties",
  "embers",
  "flat",
  "google",
  "grayscale",
  "greenscreen",
  "harmonic",
  "hopscotch",
  "isotope",
  "marrakesh",
  "mocha",
  "monokai",
  "ocean",
  "paraiso",
  "pop",
  "railscasts",
  "shapeshifter",
  "solarized",
  "summerfruit",
  "threezerotwofour",
  "tomorrow",
  "tube",
  "twilight",
];

export { CouchBaseTable, CouchBaseQuery };
