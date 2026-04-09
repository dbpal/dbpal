import {
  Box,
  Button,
  ButtonGroup,
  Chip,
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
  AiMarkdownModel,
  AiMarkdownModelSwitch,
  AppCode,
  ListenElementSize,
  LoadStoreFile,
  QueryStatus,
  SetFilterSort,
  SimpleFlexEditor,
  SliderSize,
  TableAddButton,
  ToggleChatShow,
} from "./CommonComp2.tsx";
import { ReactNode, useCallback, useContext, useEffect, useState } from "react";
import {
  DataSourceKind,
  ErrAndLoc,
  getEditorText,
  handleCopyToClipboard,
  jumpToEditorText,
  MuiDataGridData,
  QueryIDEditorChatCtx,
  resizeHandlerSize,
  secondConfirm,
  shrinkAble,
  validJson,
} from "./helper.ts";
import { v4 as uuidv4 } from "uuid";

import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import PlayArrowIcon from "@mui/icons-material/PlayArrow";
import FilterAltIcon from "@mui/icons-material/FilterAlt";
import { FaArrowsAltV } from "react-icons/fa";
import { ModalHeader, SelectPageSize } from "./CommonComp.tsx";
import { Channel, invoke } from "@tauri-apps/api/core";
import types from "./ApiStruct.ts";
import {
  dbpalQuerykey,
  EscDownEvent,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
  QueryEvent,
} from "./Common.ts";
import { editor, Position } from "monaco-editor";
import ClearIcon from "@mui/icons-material/Clear";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { QueryLog } from "./QueryLog.tsx";
import { ExportView } from "./Export.tsx";
import { QueryResp } from "./QueryResp.tsx";
import {
  useNotification,
  usePageSize,
  useQueryLogStore,
  UseSizeConfig,
  useSizeStore,
} from "./store.tsx";
import { persist } from "zustand/middleware";
import { create } from "zustand";
export enum CBQueryViewMode {
  Table = "table",
  Json = "json",
}

interface CBQueryViewState {
  viewMode: CBQueryViewMode;
  setViewMode: (mode: CBQueryViewMode) => void;
}

export const useCBQueryViewStore = create<CBQueryViewState>()(
  persist(
    (set) => ({
      // 預設值為 Json (對應你 load 裡面的邏輯)
      viewMode: CBQueryViewMode.Json,
      setViewMode: (viewMode) => set({ viewMode }),
    }),
    {
      name: "couchbase-query-view-mode",
    },
  ),
);

type Data = {
  data: { data: string; id: string }[];
  count: number;
};

function newDataDefault(): Data {
  return { count: 0, data: [] };
}

function CouchBaseTable(props: { uuid: string; table: string }) {
  const { uuid, table } = props;
  const [page, setPage] = useState(1);
  const { couchbase: pageSize, setPageSize } = usePageSize();

  const [where, setWhere] = useState("");
  const [sort, setSort] = useState("");
  const [showSetSort, setShowSetSort] = useState(false);
  const [showSetWhere, setShowSetWhere] = useState(false);
  const kind = DataSourceKind.CouchBase;
  const [resp, setResp] = useState<Data>(newDataDefault());
  const { getSize } = useSizeStore();
  const [loading, setLoading] = useState(true);
  const [queryStatus, setQueryStatus] = useState("");
  const { showNotification } = useNotification();
  const [showAdd, setShowAdd] = useState(false);
  const [addModalBox, setAddModalBox] = useState(null);

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
          showNotification({
            severity: "error",
            title: "load data failed",
            message: event.err,
          });
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
        showNotification({ severity: "error", title: "load data failed", message: e });
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
        style={{ alignItems: "center", justifyContent: "space-between" }}>
        <Stack direction="row" style={{ alignItems: "center" }}>
          <Button
            onClick={() => {
              handleReload();
            }}
            loading={loading}>
            reload
          </Button>
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
          <SliderSize config={UseSizeConfig.gridCol} />
          <SliderSize config={UseSizeConfig.editorHeight} />
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
          gridTemplateColumns: `repeat(${getSize(UseSizeConfig.gridCol)}, 1fr)`,
          paddingRight: "1.2rem",
        }}>
        {resp.data.map((item) => (
          <div
            style={{
              height: getSize(UseSizeConfig.editorHeight) + "px",
              position: "relative",
            }}
            key={item.id}>
            <CouchBaseEditor
              docid={item.id}
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
        style={{ alignItems: "center", justifyContent: "space-between" }}>
        <Stack direction="row" style={{ alignItems: "center" }}>
          <Typography color="info">
            result {resp.data.length} of {resp.count}
          </Typography>
        </Stack>

        <Stack direction="row" style={{ alignItems: "center" }}>
          <SelectPageSize
            pageSize={pageSize}
            handleChange={(s) => {
              setPageSize("couchbase", s);
            }}
            options={[2, 3, 10].map((x) => x * getSize(UseSizeConfig.gridCol))}
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

      <ListenElementSize dom={addModalBox} name="couch-addbox" set="both" />

      {showAdd && (
        <Modal
          open={showAdd}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowAdd(false);
          })}>
          <Box
            sx={{ resize: "both", ...modalStyle }}
            ref={setAddModalBox}
            width="80%"
            height="30vh">
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
  docid?: string;
}) {
  const {
    value,
    handleValue,
    handleDelete,
    handleValueButton,
    barExtra,
    warnModify,
    docid,
  } = props;
  const [jsonErr, setJsonErr] = useState<ErrAndLoc | null>(null);
  const { showNotification } = useNotification();
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
        showNotification({ severity: "success", title: `${handleValueButton} OK` });
        setChanged(false);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: `${handleValueButton} failure`,
          message: e,
        });
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
            showNotification({ severity: "success", title: "delete OK" });
          })
          .catch((e) => {
            showNotification({
              severity: "error",
              title: "delete failure",
              message: e,
            });
          })
          .finally(() => {
            setLoading(false);
          });
      }
    : undefined;

  return (
    <Stack style={{ height: "100%", flex: 1 }}>
      <SimpleFlexEditor
        value={newValue}
        handleValue={async (newValue) => {
          try {
            const x = await validJson(newValue);
            setJsonErr(x);
          } catch (e) {
            showNotification({
              severity: "error",
              title: "call valid json failed",
              message: e,
            });
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
                }}>
                {jsonErr.msg}
              </Typography>
            )}
          </Stack>
        }
      />

      <Stack direction="row" style={{ alignItems: "center" }}>
        {doModify && !jsonErr && (
          <>
            <Button
              onClick={() => {
                doModify();
              }}
              loading={loading}>
              {handleValueButton}
            </Button>
          </>
        )}
        {doDelete && (
          <Button
            onClick={() => {
              doDelete();
            }}
            color="warning">
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
              }}>
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
  return (
    <CouchBaseEditor
      warnModify={false}
      value={"{\n}"}
      barExtra=<>
        <Button
          onClick={() => {
            setId(uuidv4());
          }}
          variant="text">
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
                    edge="end">
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
          required
        />
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

interface ExecuteButtonProps {
  handleExecute: (text: string) => void; // 修正了拼写
  getText: () => string;
  disabled: boolean;
  loading?: boolean; // 保留原有的 loading 属性支持
}

export function ExecuteButton(props: ExecuteButtonProps) {
  const { handleExecute, getText, disabled, loading } = props;

  return (
    <Tooltip title="execute">
      <span>
        {/* 当 IconButton 禁用时，Tooltip 需要一个 span 包裹才能正常触发 */}
        <IconButton
          loading={loading}
          onClick={() => {
            const text = getText();
            if (!text || text.trim() === "") {
              return;
            }
            handleExecute(text);
          }}
          disabled={disabled}
          color="primary">
          <PlayArrowIcon />
        </IconButton>
      </span>
    </Tooltip>
  );
}

function CouchBaseQuery(props: { uuid: string; value?: string }) {
  const { uuid, value } = props;
  const [resp, setResp] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const { getSize } = useSizeStore();
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [queryStatus, setQueryStatus] = useState("");
  const { showNotification } = useNotification();
  const [showQueryLog, setShowQueryLog] = useState(false);
  const kind = DataSourceKind.CouchBase;
  const [executeTimes, setExecuteTimes] = useState(0);
  const [showExport, setShowExport] = useState(false);
  const { viewMode, setViewMode } = useCBQueryViewStore();
  const [tableResp, setTableResp] = useState<MuiDataGridData>(null);
  const setEditorContext = useContext(QueryIDEditorChatCtx);
  const [markdownModel, setMarkdownModel] = useState(false);
  const [markdownModelValue, setMarkdownModelValue] = useState("");

  useEffect(() => {
    setEditorContext.setEditor(editor);
  }, [editor, setEditor, setEditorContext]);

  const { save: saveQueryLog } = useQueryLogStore();
  useEffect(() => {
    if (viewMode != CBQueryViewMode.Table) {
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

    for (let obj of resp.map((x) => JSON.parse(x))) {
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

  const handleQuery = (text: string) => {
    setLoading(true);
    setResp([]);
    const channel = new Channel<QueryEvent>((event) => {
      switch (event.kind) {
        case "structure": {
          console.error("server should not emit this.", event);
          break;
        }
        case "data": {
          const xs = event.data.map((x) => x[0]);
          setResp((resp) => {
            return [...resp, ...xs];
          });
          setQueryStatus(`receive ${xs.length} rows`);
          break;
        }
        case "err":
          showNotification({
            severity: "error",
            title: "query failed",
            message: event.err,
          });
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
      maxRow: getSize(UseSizeConfig.queryLimit),
      sql: text,
      emit: channel,
    })
      .catch((e) => {
        showNotification({ severity: "error", title: "query failed", message: e });
        saveQueryLog(uuid, text, false);
      })
      .then(() => {
        saveQueryLog(uuid, text, true);
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
      autoSaveId="couchbase-query">
      <Panel
        collapsible
        collapsedSize={0}
        minSize={10}
        defaultSize={35}
        style={{ display: "flex", flexDirection: "column" }}>
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
          }}>
          <Stack direction="row" style={{ alignItems: "center" }}>
            <ExecuteButton
              disabled={markdownModel}
              getText={() => {
                return getEditorText(editor, showNotification);
              }}
              handleExecute={handleQuery}
              loading={loading}
            />
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
              {[CBQueryViewMode.Json, CBQueryViewMode.Table].map((e) => (
                <Button
                  key={e}
                  disabled={e == viewMode}
                  onClick={() => {
                    setViewMode(e);
                  }}>
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
                disabled={markdownModel}
                onClick={() => {
                  const value = getEditorText(editor, showNotification);
                  if (!value) {
                    return;
                  }
                  setShowExport(true);
                }}
                color="primary">
                <ArrowCircleDownRoundedIcon />
              </IconButton>
            </Tooltip>
            <Tooltip title="query log">
              <IconButton
                onClick={() => {
                  setShowQueryLog((prev) => !prev);
                }}
                color="primary">
                <EventNoteIcon />
              </IconButton>
            </Tooltip>

            {viewMode == CBQueryViewMode.Json && (
              <SliderSize config={UseSizeConfig.gridCol} />
            )}
            <SliderSize config={UseSizeConfig.queryLimit} />
            <ToggleChatShow />
            <AiMarkdownModelSwitch
              setMarkdownModel={setMarkdownModel}
              markdownModel={markdownModel}
            />
          </Stack>
        </Stack>

        <AiMarkdownModel
          markdownModel={markdownModel}
          setMarkdownModelValue={setMarkdownModelValue}
          editor={editor}
          appCodeHandleEdit={({ code }) => {
            setMarkdownModel(false);
            jumpToEditorText(editor, code);
          }}
          appcodeExtra={({ code }) => {
            return (
              <ExecuteButton
                disabled={false}
                getText={() => {
                  return code;
                }}
                handleExecute={handleQuery}
                loading={loading}
              />
            );
          }}
          markdownModelValue={markdownModelValue}
        />

        <div
          style={{
            flex: 1,
            display: !markdownModel ? "flex" : "none",
            flexDirection: "column",
          }}>
          <SimpleFlexEditor
            value={value}
            language={markdownModel ? "markdown" : "sql"}
            handleEditor={(e) => {
              setEditor(e);
              e.getModel()[dbpalQuerykey] = new NameAndQueryId(
                uuid,
                DataSourceKind.CouchBase,
                "",
              );
            }}
          />
        </div>
      </Panel>

      {resp.length > 0 && (
        <>
          <PanelResizeHandle
            style={{
              height: resizeHandlerSize,
            }}
            id="chat-resize-bar"
            className="resize-handle"
          />
          <Panel
            collapsible
            collapsedSize={0}
            minSize={10}
            style={{ overflow: "auto" }}>
            {viewMode == CBQueryViewMode.Json && (
              <div
                style={{
                  overflow: "auto",
                  display: "grid",
                  gridTemplateColumns: `repeat(${getSize(UseSizeConfig.gridCol)}, 1fr)`,
                  paddingRight: "1.2rem",
                }}>
                {resp.map((value, index) => {
                  return <AppCode key={index} language="json" codeBlock={value} />;
                })}
              </div>
            )}
            {viewMode == CBQueryViewMode.Table && tableResp && (
              <QueryResp data={tableResp} />
            )}
          </Panel>{" "}
        </>
      )}

      {showExport && (
        <Modal
          open={showExport}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowExport(false);
          })}>
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

export { CouchBaseTable, CouchBaseQuery };
