import {
  Button,
  Checkbox,
  Divider,
  FormControlLabel,
  IconButton,
  Stack,
  Tooltip,
  Typography,
} from "@mui/material";
import ClearIcon from "@mui/icons-material/Clear";
import { DataSourceKind, resizeHandlerSize } from "./helper";
import {
  CSSProperties,
  RefObject,
  useCallback,
  useEffect,
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
import UploadIcon from "@mui/icons-material/Upload";
import CloseIcon from "@mui/icons-material/Close";
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

function SqlCreateTable(props: {
  uuid: string;
  kind: DataSourceKind;
  fields?: CreateTableReqColumn[];
  tableName?: RefObject<string>;
}) {
  const { uuid, kind, fields: fieldsProvide, tableName: tableNameRef } = props;
  const [cols, setCols] = useState<CreateTableReqColumn[]>(
    fieldsProvide
      ? fieldsProvide
      : [
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
        ],
  );

  useEffect(() => {
    if (fieldsProvide) {
      setCols(fieldsProvide);
    }
  }, [fieldsProvide]);

  const [indexes, setIndexes] = useState<CreateIndexReq[]>([]);
  const [table, setTable] = useState("");
  const containerStyle: CSSProperties = {
    border: "2px solid grey",
    padding: "4px",
    borderRadius: "10px",
  };

  useEffect(() => {
    if (tableNameRef) {
      tableNameRef.current = table;
    }
  }, [table, tableNameRef]);

  const [loading, setLoading] = useState(false);
  const [showQuery, setShowQuery] = useState(false);
  const [editorValue, setEditorValue] = useState("");
  const tableNameDom = useRef<HTMLInputElement>(null);
  const { showNotification } = useNotification();
  const tabSize = useTabSize();

  const makeReq = useCallback(() => {
    {
      // check
      if (!table) {
        showNotification({ severity: "warning", title: "table is none" });
        tableNameDom.current.focus();
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
        <Tooltip title="auto increment">
          <FormControlLabel
            label="AI"
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
        </Tooltip>
      )}
      <Tooltip title="primary key">
        <span>
          <FormControlLabel
            label="PK"
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
  const { showNotification } = useNotification();
  const [fields, setFields] = useState<Field[]>([]);
  const [data, setData] = useState<Record<string, string>[]>();
  const tableName = useRef("");
  const [loading, setLoading] = useState(false);
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

  useEffect(() => {
    (async () => {
      try {
        if (!file) {
          return;
        }
        let fileLower = file.toLowerCase();
        if (fileLower.endsWith(".csv")) {
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
          {file && (
            <SqlCreateTable
              uuid={uuid}
              kind={kind}
              fields={fields.map((e): CreateTableReqColumn => {
                return {
                  name: e.field,
                  data_type: e.data_type,
                  nullable: "unspecified",
                  primary_key: false,
                  auto_increment: false,
                  default_value: null,
                };
              })}
              tableName={tableName}
            />
          )}
        </Stack>
      </Panel>
      <PanelResizeHandle
        style={{ width: resizeHandlerSize }}
        className="resize-handle"
      />
      {file && (
        <Panel
          style={{
            overflow: "auto",
            height: "100%",
            padding: "2px",
          }}>
          <Stack style={{ height: "100%" }}>
            <Stack direction="row">
              {file && (
                <IconButton
                  loading={loading}
                  color="success"
                  onClick={async () => {
                    if (!tableName.current) {
                      showNotification({
                        severity: "error",
                        title: "table name is null,do u have create the table",
                      });
                      return;
                    }
                    setLoading(true);
                    try {
                      await commands.tableImportFromFile(uuid, tableName.current, {
                        path: file,
                      });
                      showNotification({
                        severity: "success",
                        title: "insert data into data success",
                      });
                    } catch (e) {
                      showNotification({
                        severity: "error",
                        title: "update data failed",
                        message: e,
                      });
                    } finally {
                      setLoading(false);
                    }
                  }}>
                  <UploadIcon />
                </IconButton>
              )}
            </Stack>
            <DataGrid
              style={{ flex: 1, height: "100px" }}
              columns={fields}
              rows={data}
              getRowId={(e) => {
                return e[keyNotInTable];
              }}
              slots={{ toolbar: AppDataGridToolBar }}
            />
          </Stack>
        </Panel>
      )}
    </PanelGroup>
  );
}

export { SqlCreateTable, SqlCreateTableImportFile };
