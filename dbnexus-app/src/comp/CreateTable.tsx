import {
  Button,
  Checkbox,
  Divider,
  FormControlLabel,
  IconButton,
  InputAdornment,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import ClearIcon from "@mui/icons-material/Clear";
import { DataSourceKind } from "./helper";
import { CSSProperties, useCallback, useRef, useState } from "react";
import types from "./ApiStruct";
import AddIcon from "@mui/icons-material/Add";
import {
  ColumnNullness,
  CreateIndexReq,
  SqlDefaultValue,
  StackRowCenter,
  TypeSelect,
} from "./CommonComp2";
import CloseIcon from "@mui/icons-material/Close";
import KeyIcon from "@mui/icons-material/Key";
import { invoke } from "@tauri-apps/api/core";
import { AllKindChat } from "./Chat";
import { makeFormatFn } from "./Common";
import { useNotification, useUserSettings } from "./store";

function CreateTable(props: { uuid: string; kind: DataSourceKind }) {
  const { uuid, kind } = props;
  const [cols, setCols] = useState<types.CreateTableReqColumn[]>([
    {
      name: "id",
      data_type: "",
      auto_increment: false,
      primary_key: false,
      default_value: null,
      nullable: "none",
    },
    {
      name: "create_at",
      data_type: "",
      auto_increment: false,
      primary_key: false,
      default_value: null,
      nullable: "none",
    },
  ]);
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
  const setting = useUserSettings();
  const tableNameRef = useRef<HTMLInputElement>(null);
  const { showNotification } = useNotification();
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
    invoke<string>("create_table_req_sql", { name: uuid, req })
      .then((sql) => {
        setEditorValue("");
        setTimeout(() => {
          setEditorValue(
            makeFormatFn(kind)(sql, {
              insertSpaces: true,
              tabSize: setting.editor.tabSize,
            }),
          );
        }, 200);
      })
      .catch((e) => {
        showNotification("error", "try generate sql failed", e);
      });
  }, [kind, makeReq, setting.editor.tabSize, showNotification, uuid]);

  const handleCreate = async () => {
    const req = makeReq();
    if (!req) {
      return;
    }
    setLoading(true);
    invoke("create_table", { name: uuid, req })
      .then(() => {
        showNotification("success", "create table ok");
      })
      .catch((e) => {
        showNotification("error", "create table failed", e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack style={{ height: "100%" }}>
      <Stack style={{ ...containerStyle }} divider={<Divider />}>
        <StackRowCenter>
          <Typography variant="h5"> column </Typography>
        </StackRowCenter>

        {cols.map((col, index) => {
          return (
            <TableColumn
              index={index}
              kind={kind}
              handleClose={() => {
                setCols((cols) => {
                  return cols.filter((_, idx) => index != idx);
                });
              }}
              uuid={uuid}
              key={index}
              col={col}
              handleCol={(newCol) => {
                setCols((cols) => {
                  cols[index] = newCol;
                  return [...cols];
                });
              }}
              resetAutoInc={() => {
                cols.forEach((col) => {
                  if (col.auto_increment) {
                    col.auto_increment = false;
                    col.primary_key = false;
                  }
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
                nullable: "none",
              },
            ]);
          }}>
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
              }}>
              <AddIcon />
            </Button>
          </Stack>
          <Stack direction="row" style={{ overflow: "scroll" }} spacing={1}>
            {indexes.map((_, index) => {
              return (
                <Stack key={index} direction="row" style={{}}>
                  <CreateIndexReq
                    style={{ width: "28rem" }}
                    fields={cols.filter((e) => !!e.name).map((f) => f.name)}
                    handleReq={(ret) => {
                      indexes[index] = ret;
                      setIndexes([...indexes]);
                    }}
                    kind={kind}
                  />
                  <Button
                    onClick={() => {
                      setIndexes(indexes.filter((_, idx) => idx != index));
                    }}>
                    <ClearIcon />
                  </Button>
                </Stack>
              );
            })}
          </Stack>
        </Stack>
      )}
      <StackRowCenter>
        <Button onClick={handleCreate} loading={loading}>
          create
        </Button>
        <TextField
          value={table}
          onChange={(e) => {
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
                        edge="end">
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
            setShowQuery((e) => !e);
          }}
          variant={showQuery ? "outlined" : "contained"}>
          {showQuery ? "close editor" : "show editor"}
        </Button>
        {showQuery && <Button onClick={setCreateSql}> gen sql </Button>}
      </StackRowCenter>
      {showQuery && (
        <div
          style={{
            minHeight: "65vh",
            flex: 1,
            border: "2px solid grey",
            borderRadius: "0.8rem",
          }}>
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
  // only should have one column that is auto increment
  // impl don't need make make rerender
  resetAutoInc: () => void;
}) {
  const { col, handleCol, uuid, handleClose, kind, index, resetAutoInc } = props;
  const supportAutoInc = [
    DataSourceKind.Sqlite,
    DataSourceKind.LibSql,
    DataSourceKind.Mysql,
    DataSourceKind.TiDB,
    DataSourceKind.MariaDB,
  ].includes(kind);

  const supportNull = ![DataSourceKind.Cassandra].includes(kind);
  const supportDefault = ![DataSourceKind.Cassandra].includes(kind);

  return (
    <Stack
      direction="row"
      style={{
        alignItems: "center",
      }}>
      <IconButton onClick={handleClose}>
        <CloseIcon />
      </IconButton>
      #{index}
      {supportAutoInc && (
        <FormControlLabel
          label="auto increment"
          control=<Checkbox
            checked={col.auto_increment}
            onChange={(e) => {
              const auto_inc = e.target.checked;
              if (auto_inc) {
                resetAutoInc();
              }
              handleCol({
                ...col,
                auto_increment: auto_inc,
                primary_key: auto_inc,
              });
            }}
          />
        />
      )}
      <Tooltip title="primary key">
        <FormControlLabel
          label=<KeyIcon />
          control=<Checkbox
            disabled={col.auto_increment}
            checked={col.primary_key}
            onChange={(e) => {
              handleCol({ ...col, primary_key: e.target.checked });
            }}
          />
        />
      </Tooltip>
      <TextField
        value={col.name}
        onChange={(e) => {
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
                      edge="end">
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
        handleChange={(dt) => {
          handleCol({ ...col, data_type: dt });
        }}
        uuid={uuid}
        type={col.data_type}
        style={{ flex: 1 }}
      />
      {supportNull && (
        <ColumnNullness
          nullable={col.nullable}
          handleNullable={(e) => {
            handleCol({ ...col, nullable: e ? e : null });
          }}
        />
      )}
      {!col.auto_increment && supportDefault && (
        <SqlDefaultValue
          uuid={uuid}
          defaultValue={col.default_value}
          setDefaultValue={(dv) => {
            handleCol({ ...col, default_value: dv ? dv : null });
          }}
        />
      )}
    </Stack>
  );
}

export { CreateTable };
