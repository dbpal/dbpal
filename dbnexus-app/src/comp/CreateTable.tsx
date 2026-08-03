import {
  Button,
  Checkbox,
  Divider,
  FormControlLabel,
  IconButton,
  Stack,
  Switch,
  Tooltip,
  Typography,
} from "@mui/material";
import ClearIcon from "@mui/icons-material/Clear";
import { DataSourceKind, resizeHandlerSize } from "./helper";
import {
  CSSProperties,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import AddIcon from "@mui/icons-material/Add";
import {
  AppDataGridToolBar,
  ClearableTextField,
  ColumnNullness,
  CreateIndexView,
  SqlDefaultValue,
  StackRowCenter,
  TypeSelect,
} from "./CommonComp2";
import CloseIcon from "@mui/icons-material/Close";
import KeyIcon from "@mui/icons-material/Key";
import { AllKindChat } from "./Chat";
import { formatSqlFn, keyNotInTable } from "./Common";
import { useNotification, useTabSize } from "./store";
import {
  commands,
  CreateIndexReq,
  CreateTableReq,
  CreateTableReqColumn,
  Field,
} from "./bindings";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { SelectPath } from "./CommonComp";
import Papa from "papaparse";
import { DataGrid } from "@mui/x-data-grid";
import { readTextFile } from "@tauri-apps/plugin-fs";
import { createColumnsState } from "@mui/x-data-grid/internals";

function SqlCreateTable(props: { uuid: string; kind: DataSourceKind }) {
  const { uuid, kind } = props;
  const [cols, setCols] = useState<CreateTableReqColumn[]>([
    {
      name: "id",
      data_type: "",
      auto_increment: false,
      primary_key: false,
      default_value: null,
      nullable: "unspecified",
    },
    {
      name: "create_at",
      data_type: "",
      auto_increment: false,
      primary_key: false,
      default_value: null,
      nullable: "unspecified",
    },
  ]);
  const [indexes, setIndexes] = useState<CreateIndexReq[]>([]);
  const [table, setTable] = useState("");
  const containerStyle: CSSProperties = {
    border: "2px solid grey",
    padding: "4px",
    borderRadius: "10px",
  };
  const [loading, setLoading] = useState(false);
  const [showQuery, setShowQuery] = useState(false);
  const [editorValue, setEditorValue] = useState("");
  const tableNameRef = useRef<HTMLInputElement>(null);
  const { showNotification } = useNotification();
  const tabSize = useTabSize();

  const makeReq = useCallback(() => {
    {
      // check
      if (!table) {
        showNotification({ severity: "warning", title: "table is none" });
        tableNameRef.current.focus();
        return;
      }
      if (cols.length == 0) {
        showNotification({ severity: "error", title: "need at least one column " });
        return;
      }
      // check all colum
      for (let index = 0; index < cols.length; index++) {
        const col = cols[index];
        if (!col.name) {
          showNotification({
            severity: "warning",
            title: `#${index} column name is none`,
          });
          return;
        }
        if (!col.data_type) {
          showNotification({
            severity: "warning",
            title: `#${index} column data type is none`,
          });
          return;
        }
      }
      // check index
      for (let index = 0; index < indexes.length; index++) {
        const i = indexes[index];
        if (!i.name) {
          showNotification({
            severity: "warning",
            title: `#${index} index name is none`,
          });
          return;
        }
        if (i.columns.length == 0) {
          showNotification({
            severity: "warning",
            title: `#${index} index need select columns`,
          });
          return;
        }
      }
    }
    // ok to create
    let req: CreateTableReq = {
      name: table,
      columns: cols,
      indexes: indexes,
    };
    return req;
  }, [cols, indexes, showNotification, table]);

  const setCreateSql = useCallback(() => {
    const req = makeReq();

    commands
      .createTableReqSql(uuid, req)
      .then((sql) => {
        setEditorValue("");
        setTimeout(() => {
          setEditorValue(formatSqlFn(kind, sql, tabSize));
        }, 200);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "create sql failed",
          message: e,
        });
      });
  }, [kind, makeReq, showNotification, tabSize, uuid]);

  const handleCreate = async () => {
    const req = makeReq();
    if (!req) {
      return;
    }
    setLoading(true);
    commands
      .createTable(uuid, req)
      .then(() => {
        showNotification({ severity: "success", title: "create table ok" });
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "create table failed",
          message: e,
        });
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
              handleDelete={() => {
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
        <IconButton
          onClick={() => {
            setCols([
              ...cols,
              {
                name: "",
                data_type: "",
                auto_increment: false,
                primary_key: false,
                default_value: null,
                nullable: "unspecified",
              },
            ]);
          }}>
          <AddIcon />
        </IconButton>
      </Stack>
      {cols.length > 0 && (
        <Stack style={{ ...containerStyle }}>
          <Stack direction="row">
            <Typography variant="h5"> index </Typography>
            <IconButton
              onClick={() => {
                setIndexes([
                  ...indexes,
                  {
                    name: "",
                    columns: [],
                    kind: "index",
                  },
                ]);
              }}>
              <AddIcon />
            </IconButton>
          </Stack>
          <Stack direction="row" style={{ overflow: "scroll" }} spacing={1}>
            {indexes.map((_, index) => {
              return (
                <Stack key={index} direction="row" style={{}}>
                  <CreateIndexView
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
        <ClearableTextField
          value={table}
          onChange={(e) => {
            setTable(e.target.value);
          }}
          inputRef={tableNameRef}
          style={{ flex: 1, maxWidth: "22rem" }}
          label="table name"
          required
          onClear={() => {
            setTable("");
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

function supportAutoIncrement(kind) {
  const supportAutoInc = [
    DataSourceKind.Sqlite,
    DataSourceKind.LibSql,
    DataSourceKind.Mysql,
    DataSourceKind.TiDB,
    DataSourceKind.MariaDB,
  ].includes(kind);
  return supportAutoInc;
}

function autoIncrementFieldType(kind) {
  const typeMap = {
    [DataSourceKind.Sqlite]: "INTEGER", // SQLite 自增主键
    [DataSourceKind.LibSql]: "INTEGER", // LibSQL 兼容 SQLite
    [DataSourceKind.Mysql]: "INT", // MySQL 自增主键（或 BIGINT）
    [DataSourceKind.TiDB]: "INT", // TiDB 兼容 MySQL
    [DataSourceKind.MariaDB]: "INT", // MariaDB 兼容 MySQL
  };

  return typeMap[kind] || "INT"; // 默认返回 INT
}

function TableColumn(props: {
  col: CreateTableReqColumn;
  handleCol: (x: CreateTableReqColumn) => void;
  uuid: string;
  handleDelete?: () => void;
  kind: DataSourceKind;
  index: number;
  // only should have one column that is auto increment
  // impl don't need make make rerender
  resetAutoInc: () => void;
}) {
  const { col, handleCol, uuid, handleDelete, kind, index, resetAutoInc } = props;
  const supportAutoInc = supportAutoIncrement(kind);

  const supportNull = ![DataSourceKind.Cassandra].includes(kind);
  const supportDefault = ![DataSourceKind.Cassandra].includes(kind);

  return (
    <Stack
      direction="row"
      style={{
        alignItems: "center",
      }}>
      {handleDelete && (
        <IconButton onClick={handleDelete}>
          <CloseIcon />
        </IconButton>
      )}
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
        <span>
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
        </span>
      </Tooltip>
      <ClearableTextField
        value={col.name}
        onChange={(e) => {
          handleCol({ ...col, name: e.target.value });
        }}
        style={{ flex: 1 }}
        label="column name"
        required
        onClear={() => {
          handleCol({ ...col, name: "" });
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

function SqlCreateTableImportFile(props: { kind: DataSourceKind; uuid: string }) {
  const { uuid, kind } = props;
  const [file, setFile] = useState("");
  const [autoIncId, setAutoIncId] = useState(false);
  const [tableName, setTableName] = useState("");
  const { showNotification } = useNotification();
  const [fields, setFields] = useState<Field[]>([]);
  const [sql, setSql] = useState("");
  const [data, setData] = useState<Record<string, string>[]>();

  useEffect(() => {
    (async () => {
      try {
        if (!file) {
          return;
        }
        const fs = await commands.sqlInferFieldsFromFile(uuid, file);
        setFields(fs);
      } catch (e) {
        showNotification({
          severity: "error",
          title: "can't infer fields from file",
          message: e,
        });
      }
    })();
  }, [file, showNotification, uuid]);

  const containsId = useMemo((): boolean => {
    for (const f of fields) {
      if (f.field.toLowerCase() == "id") {
        return true;
      }
      return false;
    }
  }, [fields]);
  useEffect(() => {
    (async () => {
      if (!fields.length) {
        return "";
      }
      if (!tableName) {
        return;
      }
      let req: CreateTableReqColumn[] = [];
      if (autoIncId && !containsId) {
        req.push({
          name: "id",
          auto_increment: true,
          primary_key: true,
          nullable: "notnull",
          data_type: autoIncrementFieldType(kind),
          default_value: null,
        });
      }
      fields.forEach((f) => {
        req.push({
          name: f.field,
          data_type: f.data_type,
          auto_increment: false,
          primary_key: false,
          default_value: null,
          nullable: "unspecified",
        });
      });

      try {
        let sql = await commands.createTableReqSql(uuid, {
          name: tableName,
          indexes: [],
          columns: req,
        });
        setSql(sql);
      } catch (e) {
        showNotification({
          severity: "error",
          title: "generate sql failed",
          message: e,
        });
      }
    })();
  }, [autoIncId, containsId, fields, kind, showNotification, tableName, uuid]);

  useEffect(() => {
    (async () => {
      try {
        if (!file) {
          return;
        }
        let fileLower = file.toLowerCase();
        if (fileLower.endsWith("csv")) {
          const x = Papa.parse<Record<string, any>>(await readTextFile(file), {
            header: true,
            skipEmptyLines: true,
          });

          x.data.forEach((e, index) => {
            e[keyNotInTable] = index;
          });
          setData(x.data);
        } else {
          showNotification({ severity: "error", title: "unsupported file type" });
          return [];
        }
      } catch (e) {
        showNotification({
          severity: "error",
          title: "handle data failed",
          message: e,
        });
      }
    })();
  }, [file, showNotification]);

  return (
    <PanelGroup
      direction="horizontal"
      style={{
        height: "100%",
        width: "100%",
      }}
      autoSaveId="sql-crete-table-import-from-file">
      <Panel
        minSize={10}
        maxSize={85}
        collapsible
        collapsedSize={0}
        style={{ padding: "2px" }}>
        <Stack style={{ height: "100%" }}>
          <SelectPath path={file} setPath={setFile} required label="import file" />
          <ClearableTextField
            value={tableName}
            onChange={(e) => {
              setTableName(e.target.value);
            }}
            label="table name"
            onClear={() => {
              setTableName("");
            }}
            fullWidth
          />
          {supportAutoIncrement(kind) && !containsId && (
            <FormControlLabel
              control={
                <Switch
                  onChange={(_e, checked) => {
                    setAutoIncId(checked);
                  }}
                  checked={autoIncId}
                />
              }
              label="markdown model"
            />
          )}
          <div style={{ height: "100px", flex: 1 }}>
            <AllKindChat uuid={uuid} kind={kind} value={sql} />
          </div>
        </Stack>
      </Panel>
      <PanelResizeHandle
        style={{ width: resizeHandlerSize }}
        className="resize-handle"
      />
      <Panel
        style={{
          overflow: "auto",
          height: "100%",
          padding: "2px",
        }}>
        <DataGrid
          columns={fields}
          rows={data}
          getRowId={(e) => {
            return e[keyNotInTable];
          }}
          slots={{ toolbar: AppDataGridToolBar }}
        />
      </Panel>
    </PanelGroup>
  );
}

export { SqlCreateTable, SqlCreateTableImportFile };
