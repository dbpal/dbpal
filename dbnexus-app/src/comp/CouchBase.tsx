import {
  Button,
  ButtonGroup,
  Chip,
  Dialog,
  IconButton,
  Pagination,
  Stack,
  Tooltip,
  Typography,
} from "@mui/material";
import { UploadIcon, ArrowCircleDownRoundedIcon } from "./icons";
import {
  AiMarkdownModel,
  AiMarkdownModelSwitch,
  AppCode,
  ClearableTextField,
  FetchTimeAndLoadingStatus,
  LoadStoreFile,
  QueryStatus,
  RefreshButton,
  SetFilterSort,
  ShowQueryLog,
  SimpleFlexEditor,
  SliderSize,
  ToggleChatShow,
} from "./CommonComp2.tsx";
import React, {
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import {
  DataSourceKind,
  getEditorText,
  handleCopyToClipboard,
  jumpToEditorText,
  QueryIDEditorChatCtx,
  secondConfirm,
  shrinkAble,
  TabContent,
  DbnameContext,
  resizeHandlerSize,
} from "./helper.ts";

import {
  DeleteForeverIcon,
  PlayArrowIcon,
  FilterAltIcon,
  FaArrowsAltV,
  FaUpload,
} from "./icons";
import {
  ExportTableOnTheFilterSort,
  SelectPageSize,
  SqlExportTableOnFilterSortNotice,
  VirtualVarListLeft,
} from "./CommonComp.tsx";
import { Channel } from "@tauri-apps/api/core";
import { NameAndQueryId, uuidv4 } from "./Common.ts";
import {
  Panel,
  Group as PanelGroup,
  Separator,
  useDefaultLayout,
} from "react-resizable-panels";
import { ExportView } from "./Export.tsx";
import { QueryResp } from "./QueryResp.tsx";
import {
  CBQueryViewMode,
  useCBQueryViewStore,
  useNotification,
  usePageSize,
  useQueryLogStore,
  UseSizeConfig,
  useSizeStore,
  useTabSize,
  useTabsManage,
} from "./store.tsx";
import { commands, Field, MuiDataGridData, QueryEvent } from "./bindings.ts";
import { AddTableModal } from "./LeftTable.tsx";
import { useDebounce } from "use-debounce";
import { isEqual } from "lodash-es";
import { CouchbaseTableImportFile } from "./CreateTable.tsx";

type Data = {
  data: { data: string; id: string }[];
  count: number;
};

function newDataDefault(): Data {
  return { count: 0, data: [] };
}

function CouchBaseTable_(props: { table: string }) {
  const { dbName } = useContext(DbnameContext);
  const { table } = props;
  const [page, setPage] = useState(1);
  const { couchbase: pageSize, setPageSize } = usePageSize();
  const [filter, setFilter] = useState("");
  const [sort, setSort] = useState("");
  const [showSetSort, setShowSetSort] = useState(false);
  const [showSetWhere, setShowSetWhere] = useState(false);
  const kind = DataSourceKind.CouchBase;
  const [resp, setResp] = useState<Data>(newDataDefault());
  const { getSize } = useSizeStore();
  const [loading, setLoading] = useState(true);
  const [queryStatus, setQueryStatus] = useState("");
  const { showNotification } = useNotification();
  const tabSize = useTabSize();
  const { appendTab } = useTabsManage();

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

    commands
      .showTable(
        dbName,
        false,
        table,
        {
          filter: filter ? filter : undefined,
          sort: sort ? sort : undefined,
          page_size: pageSize,
          page: page,
          tab_size: tabSize,
        },
        c,
      )

      .catch((e) => {
        showNotification({ severity: "error", title: "load data failed", message: e });
      })
      .finally(() => {
        setLoading(false);
      });
  }, [page, pageSize, showNotification, sort, tabSize, table, dbName, filter]);
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
          <RefreshButton
            onClick={() => {
              handleReload();
            }}
            loading={loading}
          />
          <Tooltip title="filter something like 'age=18'">
            <Chip
              label={filter ? filter : "..."}
              onClick={() => {
                setShowSetWhere(true);
              }}
              style={shrinkAble}
              size="medium"
              color="primary"
              icon={<FilterAltIcon />}
              onDelete={
                filter
                  ? () => {
                      setFilter("");
                    }
                  : undefined
              }
            />
          </Tooltip>
          <Tooltip title="sort something like 'age asc'">
            <Chip
              label={sort ? sort : "..."}
              onClick={() => {
                setShowSetSort(true);
              }}
              style={shrinkAble}
              size="medium"
              color="secondary"
              variant="outlined"
              icon={<FaArrowsAltV />}
              onDelete={
                sort
                  ? () => {
                      setSort("");
                    }
                  : undefined
              }
            />
          </Tooltip>

          <AddTableModal kind={DataSourceKind.CouchBase} tableName={table} />
          <IconButton
            color="success"
            onClick={() => {
              appendTab(
                new TabContent(
                  table,
                  <CouchbaseTableImportFile hasTableName={table} />,
                  <UploadIcon />,
                  dbName,
                  kind,
                ),
              );
            }}>
            <UploadIcon />
          </IconButton>
          <ExportTableOnTheFilterSort
            isTable
            tableName={table}
            para={{ filter: filter ? filter : null, sort: sort ? sort : null }}
            extraUI=<>
              <SqlExportTableOnFilterSortNotice />
            </>
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
              modifyName="modify"
              docid={item.id}
              value={item.data}
              handleDelete={async () => {
                await commands.tableDelete(dbName, table, { id: item.id });
                setResp((resp) => {
                  return {
                    count: resp.count - 1,
                    data: resp.data.filter((y) => y.id != item.id),
                  };
                });
              }}
              warnModify
              handleValue={async (newV) => {
                await commands.tableUpdate(
                  dbName,
                  table,
                  {
                    id: item.id,
                    value: newV,
                  },
                  {},
                );

                setResp((resp) => {
                  return {
                    count: resp.count,
                    data: resp.data.map((e) =>
                      e.id == item.id ? { ...e, data: newV } : e,
                    ),
                  };
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
          value={filter}
          setValue={(v) => {
            setFilter(v);
          }}
          handleClose={() => {
            setShowSetWhere(false);
          }}
          label="filter"
          kind={kind}
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
        />
      )}
    </Stack>
  );
}

const CouchBaseTable = React.memo(CouchBaseTable_);

function CouchBaseEditor(props: {
  value: string;
  handleValue: (v: string) => Promise<void>;
  handleDelete?: () => Promise<void>;
  barExtra?: ReactNode;
  warnModify: boolean;
  docid?: string;
  modifyName: string;
}) {
  const { value, handleValue, handleDelete, barExtra, warnModify, docid, modifyName } =
    props;
  const { showNotification } = useNotification();
  const [newValue, setNewValue] = useState(value);
  const [loading, setLoading] = useState(false);
  const [newValueD] = useDebounce(newValue, 1000);
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
        showNotification({ severity: "success", title: `${modifyName} OK` });
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: `${modifyName} failure`,
          message: e,
        });
      })
      .finally(() => {
        setLoading(false);
      });
  };
  const oldObject = useMemo(() => {
    return JSON.parse(value);
  }, [value]);
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
          setNewValue(newValue);
        }}
        language="json"
      />

      <Stack direction="row" style={{ alignItems: "center" }}>
        {doModify && (
          <>
            <IconButton
              disabled={(function () {
                let x;
                try {
                  x = JSON.parse(newValueD);
                } catch {
                  return true;
                }
                return isEqual(oldObject, x);
              })()}
              onClick={() => {
                doModify();
              }}
              loading={loading}>
              <FaUpload />
            </IconButton>
          </>
        )}
        {doDelete && (
          <IconButton
            onClick={() => {
              doDelete();
            }}
            color="warning">
            <DeleteForeverIcon />
          </IconButton>
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
      </Stack>
    </Stack>
  );
}

function CouchBaseInsert({ table }) {
  const [id, setId] = useState("");
  const { dbName } = useContext(DbnameContext);
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
        <ClearableTextField
          label="id"
          value={id}
          onChange={(e) => {
            setId(e.target.value);
          }}
          onClear={() => {
            setId("");
          }}
          required
        />
      </>
      modifyName="insert"
      handleValue={async (value) => {
        if (!id.trim()) {
          throw "id is empty";
        }
        await commands.tableInsert(dbName, table, {
          id: id.trim(),
          value,
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

function CouchBaseQuery_(props: { value?: string }) {
  const { value } = props;
  const { dbName } = useContext(DbnameContext);
  const [resp, setResp] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const { getSize } = useSizeStore();

  const [queryStatus, setQueryStatus] = useState("");
  const { showNotification } = useNotification();
  const { viewMode, setViewMode } = useCBQueryViewStore();
  const [tableResp, setTableResp] = useState<MuiDataGridData>(null);
  const { editor, setEditor } = useContext(QueryIDEditorChatCtx);
  const [markdownModel, setMarkdownModel] = useState(false);
  const [matchStr, setMatchStr] = useState("");
  const [showExport, setShowExport] = useState(false);
  const { save: saveQueryLog } = useQueryLogStore();
  useEffect(() => {
    if (viewMode != CBQueryViewMode.Table) {
      return;
    }
    if (resp.length == 0) {
      return;
    }
    const fields: Field[] = [];
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
              } as Field);
            }
            d[fKey] = encodeValue(obj[key][k]);
          }
        } else {
          d[key] = encodeValue(obj[key]);
          if (-1 == fields.findIndex((x) => x.field == key)) {
            fields.push({
              field: key,
            } as Field);
          }
        }
      }
      data.push(d);
    }
    setTableResp({ fields, data: data as any, sql: "", total: null });
  }, [resp, dbName, viewMode]);

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

    commands
      .querySql(dbName, "", getSize(UseSizeConfig.couchbaseQueryLimit), text, channel)

      .catch((e) => {
        showNotification({ severity: "error", title: "query failed", message: e });
        saveQueryLog(dbName, text, false);
      })
      .then(() => {
        saveQueryLog(dbName, text, true);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  const [matchStrDelay] = useDebounce(matchStr, 1000);
  const showResp = useMemo(() => {
    return resp.filter((e) => (matchStrDelay ? e.includes(matchStrDelay) : true));
  }, [matchStrDelay, resp]);
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({
    id: "couchbase-query-panel-group",
    panelIds: ["query-panel", "resp-panel"],
  });
  return (
    <PanelGroup
      style={{ height: "100%", paddingRight: "0.7rem" }}
      orientation="vertical"
      defaultLayout={defaultLayout}
      onLayoutChanged={onLayoutChanged}>
      <Panel
        id="query-panel"
        collapsible
        collapsedSize={"0%"}
        minSize={"10%"}
        defaultSize={"35%"}
        style={{ display: "flex", flexDirection: "column" }}>
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
                return getEditorText(editor, showNotification, true);
              }}
              handleExecute={handleQuery}
              loading={loading}
            />
            <LoadStoreFile
              getQuery={() => {
                return editor.getValue();
              }}
              handleLoadFile={(c) => {
                editor.setValue(c);
              }}
              kind={DataSourceKind.CouchBase}
            />

            <Tooltip title="view model">
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
            </Tooltip>
          </Stack>
          <Stack direction="row" style={{ alignItems: "center" }}>
            {viewMode == CBQueryViewMode.Json && (
              <>
                <ClearableTextField
                  value={matchStr}
                  onChange={(e) => {
                    setMatchStr(e.target.value);
                  }}
                  onClear={() => {
                    setMatchStr("");
                  }}
                  label="match string"
                  style={{ width: "13rem" }}
                />
              </>
            )}

            <Tooltip title="export query result to a file">
              <IconButton
                loading={loading}
                disabled={markdownModel}
                onClick={() => {
                  const value = getEditorText(editor, showNotification, true);
                  if (!value) {
                    return;
                  }
                  setShowExport(true);
                }}
                color="primary">
                <ArrowCircleDownRoundedIcon />
              </IconButton>
            </Tooltip>
            <ShowQueryLog />

            <ToggleChatShow />
            <AiMarkdownModelSwitch
              setMarkdownModel={setMarkdownModel}
              markdownModel={markdownModel}
            />
            <FetchTimeAndLoadingStatus isLoading={loading} />
            <SliderSize config={UseSizeConfig.couchbaseQueryLimit} />
          </Stack>
        </Stack>

        {showExport && (
          <Dialog
            open={showExport}
            onClose={() => {
              setShowExport(false);
            }}>
            <ExportView
              supportKind={["json", "jsonline"]}
              exportOP={(kind, file, taskId, sizeEmitter) => {
                const sql = getEditorText(editor, showNotification, true);
                return commands.exportQueryResultToFile(
                  dbName,
                  taskId,
                  sql,
                  "",
                  kind,
                  file,
                  sizeEmitter,
                );
              }}
              name="export_query_result"
            />
          </Dialog>
        )}

        <AiMarkdownModel
          markdownModel={markdownModel}
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
              e.getModel()._dbpal_query_id = new NameAndQueryId(
                dbName,
                DataSourceKind.CouchBase,
                "",
              );
            }}
          />
        </div>
      </Panel>

      <Separator
        style={{
          height: resizeHandlerSize,
        }}
        id="chat-resize-bar"
        className="resize-handle"
      />

      <Panel
        collapsible
        collapsedSize={"0%"}
        minSize={"10%"}
        id="resp-panel"
        style={{ overflow: "auto" }}>
        {viewMode == CBQueryViewMode.Json && (
          <VirtualVarListLeft
            data={showResp}
            show={(d) => {
              return <AppCode codeBlock={d} language="json" highlightStr={matchStr} />;
            }}
            height="100%"
          />
        )}

        {viewMode == CBQueryViewMode.Table && tableResp && (
          <QueryResp data={tableResp} />
        )}
      </Panel>
    </PanelGroup>
  );
}

const CouchBaseQuery = React.memo(CouchBaseQuery_);

export { CouchBaseTable, CouchBaseQuery, CouchBaseInsert };
