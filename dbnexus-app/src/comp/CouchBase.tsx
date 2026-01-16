import {
  Box,
  Button,
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
import {
  QueryStatus,
  SetFilterSort,
  SimpleFlexEditor,
  SliderSize,
  syntaxHightLightTheme,
  TableAddButton,
} from "./CommonComp2";
import { ReactNode, useCallback, useContext, useEffect, useRef, useState } from "react";
import {
  DataSourceKind,
  ErrAndLoc,
  generateRandomString,
  getEditorText,
  handleCopyToClipboard,
  resizeHandlerSize,
  saveAndSetSize,
  SavePageSize,
  secondConfirm,
  SetStatusFn,
  shrinkAble,
  validJson,
} from "./helper";
import PlayArrowIcon from "@mui/icons-material/PlayArrow";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import FilterAltIcon from "@mui/icons-material/FilterAlt";
import { FaArrowsAltV } from "react-icons/fa";
import { ModalHeader, SelectPageSize } from "./CommonComp";
import { Channel, invoke } from "@tauri-apps/api/core";
import types from "./ApiStruct";
import {
  EscDownEvent,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
  QueryEvent,
  SetAutoCompletionName,
} from "./Common";
import { NotificationContext } from "./Notification";
import { Position } from "monaco-editor";
import ClearIcon from "@mui/icons-material/Clear";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
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
  const [pageSize, setPageSize] = useState(SavePageSize.instance.get());
  const [where, setWhere] = useState("");
  const [sort, setSort] = useState("");
  const [showSetSort, setShowSetSort] = useState(false);
  const [showSetWhere, setShowSetWhere] = useState(false);
  const kind = DataSourceKind.CouchBase;
  const [resp, setResp] = useState<Data>(newDataDefault());
  const [gridColCount, setGridColCount] = useState(3);
  const [editorHeight, setEditorHeight] = useState(500);
  const [loading, setLoading] = useState(true);
  const queryStatus = useRef<SetStatusFn>(null);
  const { showNotification } = useContext(NotificationContext);
  const [showAdd, setShowAdd] = useState(false);
  const [addModalBox, setAddModalBox] = useState(null);
  useEffect(() => {
    if (addModalBox) {
      const clean = saveAndSetSize(addModalBox, "couchbase-table-insert");
      return () => {
        clean();
      };
    }
  }, [addModalBox]);
  const handleReload = useCallback(async () => {
    const c = new Channel<QueryEvent>(event => {
      switch (event.kind) {
        case "structure": {
          setResp(resp => {
            return { ...resp, count: event.resp.total };
          });
          break;
        }
        case "data": {
          let news = event.data.map(x => {
            return { data: x[1], id: x[0] };
          });
          setResp(resp => {
            return { ...resp, data: [...resp.data, ...news] };
          });
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
      .catch(e => {
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
      <QueryStatus setRef={queryStatus} />
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
          {/* <Typography color="info"> order by </Typography> */}
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
            handleValue={v => {
              setGridColCount(v);
            }}
            storeKey="couchase-table-view-column-count"
            name="column"
          />
          <SliderSize
            name="height"
            min={300}
            max={1000}
            notice="mongodb editor height"
            step={10}
            handleValue={v => {
              setEditorHeight(v);
            }}
            storeKey="couchbase-table-editor-height"
          />
        </Stack>
      </Stack>
      <div
        style={{
          flex: 1,
          height: "100px",
          overflowY: "auto",
          overflowX: "scroll",
          display: "grid",
          gridTemplateColumns: `repeat(${gridColCount}, 1fr)`,
          paddingRight: "1.2rem",
        }}
      >
        {resp.data.map(item => (
          <div
            style={{ height: editorHeight + "px", position: "relative" }}
            key={item.id}
          >
            <Stack
              style={{
                position: "absolute",
                zIndex: 14,
                left: 0,
                top: 0,
                opacity: 0.6,
              }}
            >
              <Tooltip title="copy id">
                <IconButton
                  onClick={() => {
                    handleCopyToClipboard(item.id, showNotification);
                  }}
                >
                  <ContentCopyIcon />
                </IconButton>
              </Tooltip>
              <Typography style={{ writingMode: "vertical-rl" }}>{item.id} </Typography>
            </Stack>
            <CouchBaseEditor
              handleValueButton="modify"
              value={item.data}
              handleDelete={async () => {
                await invoke("table_delete", {
                  name: uuid,
                  tableName: table,
                  wheres: { id: item.id },
                });
                setResp(resp => {
                  return {
                    count: resp.count - 1,
                    data: resp.data.filter(y => y.id != item.id),
                  };
                });
              }}
              warnModify
              handleValue={async newV => {
                return invoke("table_update", {
                  name: uuid,
                  tableName: table,
                  values: {
                    id: item.id,
                    value: newV,
                  },
                  wheres: {},
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
        <Stack direction="row">
          <Typography color="info">
            result {resp.data.length} of {resp.count}
          </Typography>
        </Stack>

        <Stack direction="row" style={{ alignItems: "center" }}>
          <SelectPageSize
            pageSize={pageSize}
            handleChange={s => {
              setPageSize(s);
            }}
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
          setValue={v => {
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
          setValue={v => {
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
}) {
  const { value, handleValue, handleDelete, handleValueButton, barExtra, warnModify } =
    props;
  const [newValue, setNewValue] = useState(value);
  const [jsonErr, setJsonErr] = useState<ErrAndLoc | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [jsonErrPos, setJsonErrPos] = useState<Position>(null);
  useEffect(() => {
    (async () => {
      try {
        const x = await validJson(newValue);
        setJsonErr(x);
      } catch (e) {
        showNotification("error", "call valid json failed", e);
        return;
      }
    })();
  }, [newValue, showNotification]);
  const [loading, setLoading] = useState(false);
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
      })
      .catch(e => {
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
          .catch(e => {
            showNotification("error", "delete failure", e);
          })
          .finally(() => {
            setLoading(false);
          });
      }
    : undefined;
  return (
    <Stack style={{ height: "100%" }}>
      <SimpleFlexEditor
        value={newValue}
        handleValue={v => {
          setNewValue(v);
        }}
        language="json"
        errPos={jsonErrPos}
        barLeft=<Stack direction="row">
          <Button
            onClick={() => {
              doModify();
            }}
            loading={loading}
          >
            {handleValueButton}
          </Button>
          {doDelete && (
            <Button
              onClick={() => {
                doDelete();
              }}
              color="warning"
            >
              delete
            </Button>
          )}
          {barExtra}
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
      />
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
            setId(generateRandomString());
          }}
          variant="text"
        >
          gen
        </Button>
        <TextField
          label="id"
          value={id}
          onChange={e => {
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
      </>
      handleValueButton={"insert"}
      handleValue={async value => {
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

function CouchBaseQuery(props: { uuid: string; value?: string }) {
  const { uuid, value } = props;
  const [resp, setResp] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [gridColCount, setGridColCount] = useState(3);
  const [dataDivHeight, setDataDivHeight] = useState(200);
  const [editor, setEditor] = useState(null);
  const maxRow = useRef(2000);
  const queryStatus = useRef<SetStatusFn>(null);
  const { showNotification } = useContext(NotificationContext);
  const handleQuery = () => {
    let text = getEditorText(editor, showNotification);
    if (!text) {
      return;
    }
    setLoading(true);
    setResp([]);
    let c = new Channel<QueryEvent>(e => {
      switch (e.kind) {
        case "structure": {
          console.error("server should not emit this.", e);
          break;
        }
        case "data": {
          const xs = e.data.map(x => x[0]);
          setResp(resp => {
            return [...resp, ...xs];
          });
          break;
        }
      }
    });
    invoke("query_sql", {
      name: uuid,
      queryId: "",
      maxRow: maxRow.current,
      sql: text,
      emit: c,
    })
      .catch(e => {
        showNotification("error", "query failed", e);
      })
      .finally(() => {
        setLoading(false);
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
        <QueryStatus setRef={queryStatus} />
        <Stack
          direction="row"
          style={{
            alignItems: "center",
            justifyContent: "space-between",
            paddingBottom: "0.5rem",
          }}
        >
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
          <Stack direction="row">
            <SliderSize
              min={1}
              max={5}
              notice="column count"
              step={1}
              handleValue={v => {
                setGridColCount(v);
              }}
              storeKey="couchbase-json-column-size"
              name="column"
            />
            <SliderSize
              name="height"
              min={50}
              max={1000}
              notice="data div heigth"
              step={10}
              handleValue={v => {
                setDataDivHeight(v);
              }}
              storeKey="couchbase-table-editor-height"
            />
            <SliderSize
              storeKey="couchbase-sql-query"
              min={1000}
              max={100000}
              step={1000}
              handleValue={v => {
                maxRow.current = v;
              }}
              notice=<Typography>
                limit max rows can be query.we don&apos;t want OOM!!!😀
                <br />
                dbpal use Stream technology to query data,so we can query large data
              </Typography>
            />
          </Stack>
        </Stack>
        <SimpleFlexEditor
          value={value}
          language="sql"
          handleEditor={e => {
            setEditor(e);
            SetAutoCompletionName.set(
              e.getModel().id,
              new NameAndQueryId(uuid, DataSourceKind.CouchBase),
            );
          }}
        />
      </Panel>
      {resp.length > 0 && (
        <PanelResizeHandle
          style={{
            height: resizeHandlerSize,
          }}
          id="chat-resize-bar"
          className="resize-handle"
        />
      )}

      {resp.length > 0 && (
        <Panel collapsible collapsedSize={0} minSize={10}>
          <div
            style={{
              overflow: "auto",
              display: "grid",
              gridTemplateColumns: `repeat(${gridColCount}, 1fr)`,
              paddingRight: "1.2rem",
            }}
          >
            {resp.map((jsonStr, index) => {
              return (
                <div
                  style={{
                    height: dataDivHeight + "px",
                    overflow: "auto",
                    // border: "2px solid",
                  }}
                  key={index}
                >
                  <SyntaxHighlighter
                    style={syntaxHightLightTheme()} // Apply the chosen theme
                    language={"json"}
                    PreTag="div"
                    // style={{ height: "100%" }}
                  >
                    {jsonStr}
                  </SyntaxHighlighter>
                </div>
              );
            })}
          </div>
        </Panel>
      )}
    </PanelGroup>
  );
}

export { CouchBaseTable, CouchBaseQuery };
