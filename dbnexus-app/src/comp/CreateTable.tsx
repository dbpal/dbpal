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
import { DataSourceKind, isExcelFile, resizeHandlerSize } from "./helper";
import InfoIcon from "@mui/icons-material/Info";
import {
  CSSProperties,
  RefObject,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import AddIcon from "@mui/icons-material/Add";
import xlsxWithHeaderImg from "../../public/xlsx_with_header.png";
import {
  AppCode,
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
import { readTextFile, readTextFileLines } from "@tauri-apps/plugin-fs";
import { BSON, EJSON } from "bson";

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

function SqlCreateTableImportFileNotice() {
  return (
    <Tooltip
      slotProps={{
        tooltip: {
          style: { maxWidth: "40vw" },
        },
      }}
      title=<Stack style={{ width: "40vw" }}>
        <Typography>support file type:json,jsonline,csv,excel file types.</Typography>
        <Typography> 1. u need select a file </Typography>
        <Typography> 2. u need create table from the left panel</Typography>
        <Typography>
          3. then upload data from the right panel,notice that the table name will be
          used for import data from input of the left panel
        </Typography>
        <AppCode
          language="csv"
          extra={() => {
            return <> csv examples</>;
          }}
          codeBlock={`name,age
john,123`}
        />
        <AppCode
          language="json"
          extra={() => {
            return <> json examples</>;
          }}
          codeBlock={`[
  {
      "name":"john",
      "age" :112
  },
  ...
]`}
        />
        <AppCode
          language="jsonline"
          extra={() => {
            return <> json examples</>;
          }}
          codeBlock={`{"name":"john","age" :112}
...`}
        />
        <Typography> xlsx file must have headers </Typography>
        <img src={xlsxWithHeaderImg} />
      </Stack>>
      <InfoIcon color="info" />
    </Tooltip>
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

  const maxRow = 100;

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
            preview: maxRow,
          });
          x.data.forEach((e, index) => {
            e[keyNotInTable] = index;
          });
          setData(x.data);
        } else if (fileLower.endsWith(".json")) {
          // json
          const data: any[] = JSON.parse(await readTextFile(file));
          if (!Array.isArray(data)) {
            showNotification({
              severity: "error",
              title: "json file should contains array.",
            });
          }
          const dataX = [];

          for (let index = 0; index < data.length; index++) {
            const d = data[index];
            if (!d) {
              continue;
            }
            if (typeof d != "object") {
              showNotification({
                severity: "error",
                title: "data is not a object",
                message: `${JSON.stringify(d)} is not object`,
              });
              continue;
            }
            d[keyNotInTable] = dataX.push(d);
            if (dataX.length >= maxRow) {
              break;
            }
          }
          setData(dataX);
        } else if (fileLower.endsWith(".jsonl")) {
          const data = [];
          let line_num = 0;
          for await (const line of await readTextFileLines(file)) {
            line_num++;
            if (!line.trim()) {
              continue;
            }
            let x = JSON.parse(line);
            if (x && typeof x == "object") {
              x[keyNotInTable] = line_num;
              data.push(x);

              if (data.length >= maxRow) {
                break;
              }
            } else {
              showNotification({
                severity: "error",
                title: `line ${line_num} is not object`,

                message: `line content:${line} `,
              });
            }
          }
          setData(data);
        } else if (isExcelFile(fileLower)) {
          const data = await commands.readExcelFilePreview(file, maxRow);
          setFields(data.fields);
          data.data.forEach((e, index) => {
            //@ts-ignore
            e[keyNotInTable] = index;
          });

          setData(data.data);
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
      <Panel minSize={10} collapsible collapsedSize={0} style={{ padding: "2px" }}>
        <Stack style={{ height: "100%", overflow: "auto" }}>
          <Stack direction="row" style={{ alignItems: "center" }}>
            <SqlCreateTableImportFileNotice />
            <SelectPath path={file} setPath={setFile} required label="import file" />
          </Stack>

          {file && (
            <div style={{ flex: 1, height: "100px" }}>
              <SqlCreateTable
                uuid={uuid}
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
                kind={kind}
              />
            </div>
          )}
        </Stack>
      </Panel>
      <PanelResizeHandle
        style={{ width: resizeHandlerSize }}
        className="resize-handle"
      />
      {file && (
        <Panel
          minSize={10}
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
                        mongo_data: null,
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

function MongoCreateTableImportFile(props: { uuid: string; hasTableName?: string }) {
  const { uuid, hasTableName } = props;
  const [file, setFile] = useState("");
  const { showNotification } = useNotification();
  const [data, setData] = useState<string[]>([]);
  const [tableName, setTableName] = useState(hasTableName);
  const [loading, setLoading] = useState(false);

  const maxRow = 10;

  useEffect(() => {
    (async () => {
      try {
        if (!file) {
          return;
        }

        let fileLower = file.toLowerCase();
        if (fileLower.endsWith(".jsonl")) {
          const data = [];
          let line_num = 0;
          for await (const line of await readTextFileLines(file)) {
            line_num++;
            if (!line.trim()) {
              continue;
            }
            let obj = EJSON.parse(line);
            if (obj && typeof obj == "object") {
              data.push(EJSON.stringify(obj, null, 2));
              if (data.length >= maxRow) {
                break;
              }
            } else {
              showNotification({
                severity: "error",
                title: `line ${line_num} is not object`,
                message: `line content:${line} `,
              });
            }
          }
          setData(data);
        } else if (fileLower.endsWith(".json")) {
          const data = [];
          const parsed: any[] = EJSON.parse(await readTextFile(file));
          if (!Array.isArray(parsed)) {
            showNotification({
              severity: "error",
              title: "this file should contains a array",
            });
            return;
          }

          for (let index = 0; index < parsed.length; index++) {
            let x = parsed[index];
            if (x && typeof x == "object") {
              data.push(EJSON.stringify(x, null, 2));
            } else {
              showNotification({
                severity: "error",
                title: `#${index} is not a object`,
              });
            }
            if (data.length >= maxRow) {
              break;
            }
          }
          setData(data);
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

  const makeData = useCallback(async (): Promise<Uint8Array<ArrayBufferLike>[]> => {
    const pathLower = file.toLocaleLowerCase();

    if (pathLower.endsWith(".jsonl")) {
      const ret = [];
      for await (const line of await readTextFileLines(file)) {
        ret.push(BSON.serialize(EJSON.parse(line)));
      }
      return ret;
    } else if (pathLower.endsWith(".json")) {
      // xxx
      const ret = [];
      const data: any[] = EJSON.parse(await readTextFile(file));
      if (!Array.isArray(data)) {
        showNotification({
          severity: "error",
          title: "this file should contains a array",
        });
        return;
      }
      for (let index = 0; index < data.length; index++) {
        let x = data[index];
        if (x && typeof x == "object") {
          ret.push(BSON.serialize(x));
        } else {
          showNotification({ severity: "error", title: `#${index} is not a object` });
        }
      }
      return ret;
    } else {
      showNotification({ severity: "error", title: "unsupported file type" });
    }
  }, [file, showNotification]);

  return (
    <Stack
      style={{
        height: "100%",
        width: "100%",
        overflow: "auto",
      }}>
      <Stack direction="row">
        <IconButton
          loading={loading}
          onClick={async () => {
            try {
              if (!tableName) {
                showNotification({
                  severity: "error",
                  message: "table name is not set",
                });
                return;
              }
              let data = null;
              try {
                data = await makeData();
              } catch (e) {
                showNotification({
                  severity: "error",
                  title: "make data for mongodb failed",
                  message: e,
                });
                return;
              }
              if (!data) {
                return;
              }
              setLoading(true);
              await commands.tableImportFromFile(uuid, tableName, {
                path: file,
                mongo_data: data,
              });
              showNotification({ severity: "success", title: "upload data ok" });
            } catch (e) {
              showNotification({
                severity: "error",
                title: "upload data failed",
                message: e,
              });
            } finally {
              setLoading(false);
            }
          }}>
          <UploadIcon />
        </IconButton>

        <ClearableTextField
          disabled={!!hasTableName}
          value={tableName}
          onChange={(e) => {
            setTableName(e.target.value);
          }}
          onClear={() => {
            if (hasTableName) {
              return;
            }
            setTableName("");
          }}
          style={{ flex: 1 }}
          label="table name"
        />

        <SelectPath path={file} setPath={setFile} required label="import file" />
      </Stack>

      {data.map((x, index) => (
        <AppCode codeBlock={x} language="json" key={index} />
      ))}
    </Stack>
  );
}

function CouchbaseCreateTableImportFile(props: {
  uuid: string;
  hasTableName?: string;
}) {
  const { uuid, hasTableName } = props;
  const [file, setFile] = useState("");
  const { showNotification } = useNotification();
  const [data, setData] = useState<string[]>([]);
  const [tableName, setTableName] = useState(hasTableName);
  const [loading, setLoading] = useState(false);

  const maxRow = 10;

  useEffect(() => {
    (async () => {
      try {
        if (!file) {
          return;
        }

        let fileLower = file.toLowerCase();
        if (fileLower.endsWith(".jsonl")) {
          const data = [];
          let line_num = 0;
          for await (const line of await readTextFileLines(file)) {
            line_num++;
            if (!line.trim()) {
              continue;
            }
            let obj = JSON.parse(line);
            if (obj && typeof obj == "object") {
              data.push(EJSON.stringify(obj, null, 2));
              if (data.length >= maxRow) {
                break;
              }
            } else {
              showNotification({
                severity: "error",
                title: `line ${line_num} is not object`,
                message: `line content:${line} `,
              });
            }
          }
          setData(data);
        } else if (fileLower.endsWith(".json")) {
          const data = [];
          const parsed: any[] = JSON.parse(await readTextFile(file));
          if (!Array.isArray(parsed)) {
            showNotification({
              severity: "error",
              title: "this file should contains a array",
            });
            return;
          }

          for (let index = 0; index < parsed.length; index++) {
            let x = parsed[index];
            if (x && typeof x == "object") {
              data.push(EJSON.stringify(x, null, 2));
            } else {
              showNotification({
                severity: "error",
                title: `#${index} is not a object`,
              });
            }
            if (data.length >= maxRow) {
              break;
            }
          }
          setData(data);
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
    <Stack
      style={{
        height: "100%",
        width: "100%",
        overflow: "auto",
      }}>
      <Stack direction="row">
        <IconButton
          loading={loading}
          onClick={async () => {
            try {
              if (!tableName) {
                showNotification({
                  severity: "error",
                  message: "table name is not set",
                });
                return;
              }
              setLoading(true);
              await commands.tableImportFromFile(uuid, tableName, {
                path: file,
              });
              showNotification({ severity: "success", title: "upload data ok" });
            } catch (e) {
              showNotification({
                severity: "error",
                title: "upload data failed",
                message: e,
              });
            } finally {
              setLoading(false);
            }
          }}>
          <UploadIcon />
        </IconButton>

        <ClearableTextField
          disabled={!!hasTableName}
          value={tableName}
          onChange={(e) => {
            setTableName(e.target.value);
          }}
          onClear={() => {
            if (hasTableName) {
              return;
            }
            setTableName("");
          }}
          style={{ flex: 1 }}
          label="table name"
        />

        <SelectPath path={file} setPath={setFile} required label="import file" />
      </Stack>

      {data.map((x, index) => (
        <AppCode codeBlock={x} language="json" key={index} />
      ))}
    </Stack>
  );
}

export {
  SqlCreateTable,
  SqlCreateTableImportFile,
  MongoCreateTableImportFile,
  CouchbaseCreateTableImportFile,
};
