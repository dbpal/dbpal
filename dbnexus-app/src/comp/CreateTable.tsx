import {
  Button,
  Checkbox,
  FormControlLabel,
  IconButton,
  InputAdornment,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import ClearIcon from "@mui/icons-material/Clear";
import { DataSourceKind, SettingContext } from "./helper";
import {
  CSSProperties,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import types from "./ApiStruct";
import AddIcon from "@mui/icons-material/Add";
import {
  ColumnNullness,
  CreateIndexReq,
  SqlDefaultValue,
  TypeSelect,
} from "./CommonComp2";
import CloseIcon from "@mui/icons-material/Close";
import KeyIcon from "@mui/icons-material/Key";
import { NotificationContext } from "./Notification";
import { invoke } from "@tauri-apps/api/core";
import { AllKindChat } from "./Chat";
import { makeFormatFn } from "./Common";

function CreateTable(props: { uuid: string; kind: DataSourceKind }) {
  const { uuid, kind } = props;
  const [cols, setCols] = useState<types.CreateTableReqColumn[]>([]);
  const [indexes, setIndexes] = useState<types.CreateIndexReq[]>([]);
  const [table, setTable] = useState("");
  const containerStyle: CSSProperties = {
    border: "2px solid grey",
    padding: "4px",
    borderRadius: "10px",
  };
  const [loading, setLoading] = useState(false);
  const [showQuery, setShowQuery] = useState(false);
  const [editorValue, setEditorValue] = useState("");
  const setting = useContext(SettingContext);

  const tableNameRef = useRef<HTMLInputElement>(null);
  const { showNotification } = useContext(NotificationContext);

  const makeReq = useCallback(() => {
    {
      // check
      if (!table) {
        showNotification("warning", "table is none");
        tableNameRef.current.focus();
        return;
      }
      if (cols.length == 0) {
        showNotification("error", "need at least one column ");
        return;
      }
      // check all colum
      for (let index = 0; index < cols.length; index++) {
        const col = cols[index];
        if (!col.name) {
          showNotification("warning", `#${index} column name is none`);
          return;
        }
        if (!col.data_type) {
          showNotification("warning", `#${index} column data type is none`);
          return;
        }
      }
      // check index
      for (let index = 0; index < indexes.length; index++) {
        const i = indexes[index];
        if (!i.name) {
          showNotification("warning", `#${index} index name is none`);
          return;
        }
        if (i.columns.length == 0) {
          showNotification("warning", `#${index} index need select columns`);
          return;
        }
      }
    }
    // ok to create
    let req: types.CreateTableReq = {
      name: table,
      columns: cols,
      indexes: indexes,
    };
    return req;
  }, [cols, indexes, showNotification, table]);
  const setCreateSql = useCallback(() => {
    const req = makeReq();
    if (!req) {
      return;
    }
    invoke<string[]>("create_table_req_sql", { name: uuid, req })
      .then(e => {
        setEditorValue("");
        const sql = e.join(";\n");
        setEditorValue(
          makeFormatFn(kind)(sql, {
            insertSpaces: true,
            tabSize: setting.editor.tabSize,
          }),
        );
      })
      .catch(e => {
        showNotification("error", "try generate sql failed", e);
      });
  }, [kind, makeReq, setting.editor.tabSize, showNotification, uuid]);
  const handleCreate = async () => {
    const req = makeReq();
    setLoading(true);
    invoke("create_table", { name: uuid, req })
      .then(() => {
        showNotification("success", "create table ok");
      })
      .catch(e => {
        showNotification("error", "create table failed", e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack>
      <Stack style={{ ...containerStyle }}>
        <Typography variant="h5"> column </Typography>
        {cols.map((col, index) => {
          return (
            <TableColumn
              index={index}
              kind={kind}
              handleClose={() => {
                setCols(cols => {
                  return cols.filter((_, idx) => index != idx);
                });
              }}
              uuid={uuid}
              key={index}
              col={col}
              handleCol={newCol => {
                setCols(cols => {
                  cols[index] = newCol;
                  return [...cols];
                });
              }}
            />
          );
        })}
        <Button
          onClick={() => {
            setCols([
              ...cols,
              {
                name: "",
                data_type: "",
                auto_increment: false,
                primary_key: false,
                default_value: null,
              } as types.CreateTableReqColumn,
            ]);
          }}
        >
          <AddIcon />
        </Button>
      </Stack>
      {cols.length > 0 && (
        <Stack style={{ ...containerStyle }}>
          <Stack direction="row">
            <Typography variant="h5"> index </Typography>
            <Button
              onClick={() => {
                setIndexes([
                  ...indexes,
                  {
                    name: "",
                    columns: [],
                    kind: "index",
                  } as types.CreateIndexReq,
                ]);
              }}
            >
              <AddIcon />
            </Button>
          </Stack>
          <Stack direction="row" style={{ overflow: "scroll" }} spacing={1}>
            {indexes.map((_, index) => {
              return (
                <Stack key={index} direction="row" style={{}}>
                  <CreateIndexReq
                    style={{ width: "28rem" }}
                    fields={cols.filter(e => !!e.name).map(f => f.name)}
                    handleReq={ret => {
                      indexes[index] = ret;
                      setIndexes([...indexes]);
                    }}
                    kind={kind}
                  />
                  <Button
                    onClick={() => {
                      setIndexes(indexes.filter((_, idx) => idx != index));
                    }}
                  >
                    <CloseIcon> </CloseIcon>
                  </Button>
                </Stack>
              );
            })}
          </Stack>
        </Stack>
      )}

      <Stack direction="row">
        <Button onClick={handleCreate} loading={loading}>
          create
        </Button>
        <TextField
          value={table}
          onChange={e => {
            setTable(e.target.value);
          }}
          inputRef={tableNameRef}
          style={{ flex: 1, maxWidth: "22rem" }}
          label="table name"
          required
          slotProps={{
            input: {
              endAdornment: (
                <>
                  {table && (
                    <InputAdornment position="end">
                      <IconButton
                        onClick={() => {
                          setTable("");
                        }}
                        edge="end"
                      >
                        <ClearIcon fontSize="small" />
                      </IconButton>
                    </InputAdornment>
                  )}
                </>
              ),
            },
          }}
        />

        <Button
          onClick={() => {
            if (!showQuery) {
              setCreateSql();
            }
            setShowQuery(e => !e);
          }}
          variant="outlined"
        >
          {showQuery ? "close query" : "show query"}
        </Button>
        {showQuery && <Button onClick={setCreateSql}> gen sql </Button>}
      </Stack>
      {showQuery && (
        <div style={{ height: "45vh" }}>
          <AllKindChat value={editorValue} kind={kind} uuid={uuid} />
        </div>
      )}
    </Stack>
  );
}

function TableColumn(props: {
  col: types.CreateTableReqColumn;
  handleCol: (x: types.CreateTableReqColumn) => void;
  uuid: string;
  handleClose: () => void;
  kind: DataSourceKind;
  index: number;
}) {
  const { col, handleCol, uuid, handleClose, kind, index } = props;
  const supportAutoInc = [
    DataSourceKind.Sqlite,
    DataSourceKind.LibSql,
    DataSourceKind.Mysql,
    DataSourceKind.TiDB,
    DataSourceKind.MariaDB,
  ].includes(kind);
  return (
    <Stack
      direction="row"
      style={{ alignItems: "center" }}
      // divider="|"
    >
      #{index}
      {supportAutoInc && (
        <FormControlLabel
          label="auto increment"
          control=<Checkbox
            checked={col.auto_increment}
            onChange={e => {
              handleCol({ ...col, auto_increment: e.target.checked });
            }}
          />
        />
      )}
      <Tooltip title="primary key">
        <FormControlLabel
          label=<KeyIcon />
          control=<Checkbox
            checked={col.primary_key}
            onChange={e => {
              handleCol({ ...col, primary_key: e.target.checked });
            }}
          />
        />
      </Tooltip>
      <TextField
        value={col.name}
        onChange={e => {
          handleCol({ ...col, name: e.target.value });
        }}
        style={{ flex: 1 }}
        label="column name"
        required
        slotProps={{
          input: {
            endAdornment: (
              <>
                {col.name && (
                  <InputAdornment position="end">
                    <IconButton
                      onClick={() => {
                        handleCol({ ...col, name: "" });
                      }}
                      edge="end"
                    >
                      <ClearIcon fontSize="small" />
                    </IconButton>
                  </InputAdornment>
                )}
              </>
            ),
          },
        }}
      />
      <TypeSelect
        handleChange={dt => {
          handleCol({ ...col, data_type: dt });
        }}
        uuid={uuid}
        type={col.data_type}
        style={{ flex: 1 }}
      />
      <ColumnNullness
        nullness={col.nullable}
        handleNullness={e => {
          handleCol({ ...col, nullable: e ? e : null });
        }}
      />
      <SqlDefaultValue
        uuid={uuid}
        defaultValue={col.default_value}
        setDefaultValue={dv => {
          handleCol({ ...col, default_value: dv ? dv : null });
        }}
      />
      <IconButton onClick={handleClose}>
        <CloseIcon />
      </IconButton>
    </Stack>
  );
}

export { CreateTable };
