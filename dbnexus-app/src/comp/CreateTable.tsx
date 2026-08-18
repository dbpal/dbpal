import {
  Button,
  Checkbox,
  Dialog,
  Divider,
  FormControlLabel,
  IconButton,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import ClearIcon from "@mui/icons-material/Clear";
import {
  DataSourceKind,
  isCsvFile,
  isExcelFile,
  resizeHandlerSize,
  DbnameContext,
  setDifference,
} from "./helper";
import {
  CSSProperties,
  RefObject,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import AddIcon from "@mui/icons-material/Add";
import xlsxWithHeaderImg from "../../public/xlsx_with_header.png";
import {
  AppCode,
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
  MuiDataGridData,
} from "./bindings";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { LongNotice, SelectPath } from "./CommonComp";
import Papa from "papaparse";
import { DataGrid } from "@mui/x-data-grid";
import { readTextFile, readTextFileLines } from "@tauri-apps/plugin-fs";
import { BSON, EJSON } from "bson";
import { openUrl } from "@tauri-apps/plugin-opener";
import { showTable } from "./handlers";

function SqlCreateTable(props: {
  fields?: CreateTableReqColumn[];
  tableName?: RefObject<string>;
}) {
  const { dbName, kind } = useContext(DbnameContext);
  const { fields: fieldsProvide, tableName: tableNameRef } = props;
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
      .createTableReqSql(dbName, req)
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
  }, [kind, makeReq, showNotification, tabSize, dbName]);

  const handleCreate = async () => {
    const req = makeReq();
    if (!req) {
      return;
    }
    setLoading(true);
    commands
      .createTable(dbName, req)
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
          <AllKindChat value={editorValue} />
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
  handleDelete?: () => void;
  kind: DataSourceKind;
  index: number;
  // only should have one column that is auto increment
  // impl don't need make make rerender
  resetAutoInc: () => void;
}) {
  const { col, handleCol, handleDelete, kind, index, resetAutoInc } = props;
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
          defaultValue={col.default_value}
          setDefaultValue={(dv) => {
            handleCol({ ...col, default_value: dv ? dv : null });
          }}
        />
      )}
    </Stack>
  );
}

function SqlTableImportFileNotice() {
  return (
    <LongNotice>
      <Typography>
        Supported file types: JSON, JSONL, CSV, TSV, and Excel formats.
      </Typography>
      <Typography>1. Select a file to upload.</Typography>
      <Typography>2. Create a table using the left panel(if need).</Typography>
      <Typography>
        3. Upload your data via the right panel. Note that the table name must match the{" "}
        <mark>input</mark> value provided in the left panel(if we need create table).
      </Typography>
      <AppCode
        language="csv"
        extra={() => {
          return <> csv examples(need headers)</>;
        }}
        codeBlock={`name,age
john,123`}
      />
      <AppCode
        language="tsv"
        extra={() => {
          return <> tsv examples(need headers)</>;
        }}
        codeBlock={`name	age
john	123`}
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
          return <> jsonline examples</>;
        }}
        codeBlock={`{"name":"john","age" :112}
...`}
      />
      <Typography> xlsx file must have headers </Typography>
      <img src={xlsxWithHeaderImg} />{" "}
    </LongNotice>
  );
}

function SqlCreateTableImportFile(props: { hasTableName?: string }) {
  const { hasTableName } = props;

  const { dbName } = useContext(DbnameContext);
  const [file, setFile] = useState("");
  const { showNotification } = useNotification();
  const [previewFields, setPreviewFields] = useState<Field[]>([]);
  const [previewData, setPreViewData] = useState<Record<string, string>[]>();
  const tableName = useRef(hasTableName ? hasTableName : "");
  const [loading, setLoading] = useState(false);
  const [csvDelimiter, setCsvDelimiter] = useState(",");
  const [hasTableResp, setHasTableResp] = useState<MuiDataGridData>({
    total: 0,
    fields: [],
    data: [],
    sql: null,
  });
  const [insertSqlSamples, setInsertSqlSamples] = useState<string[]>(null);

  const fileLower = useMemo(() => {
    return file.toLocaleLowerCase();
  }, [file]);

  const realCsvDelimiter = useMemo(() => {
    return fileLower.endsWith(".tsv") ? "\t" : csvDelimiter;
  }, [csvDelimiter, fileLower]);
  useEffect(() => {
    (async () => {
      try {
        if (!file) {
          return;
        }
        if (isCsvFile(file, false)) {
          if (!csvDelimiter) {
            return;
          }
        }
        const fs = await commands.sqlInferFieldsFromFile(
          dbName,
          file,
          realCsvDelimiter,
        );
        setPreviewFields(fs);
      } catch (e) {
        showNotification({
          severity: "error",
          title: "can't infer fields from file",
          message: e,
        });
      }
    })();
  }, [file, showNotification, dbName, csvDelimiter, fileLower, realCsvDelimiter]);

  const maxRow = 100;

  useEffect(() => {
    if (!hasTableName) {
      return;
    }
    showTable({
      dbName,
      para: {
        where_str: null,
        order_by: null,
        page: 1,
        page_size: 100,
      },
      name: hasTableName,
      isView: false,
      setStatus: () => {},
      showNotification: undefined,
      handleResult: function (x): void {
        x.data.forEach((e, index) => {
          e[keyNotInTable] = index + "";
        });
        setHasTableResp(x);
      },
    });
  }, [dbName, hasTableName]);

  useEffect(() => {
    (async () => {
      try {
        if (!file) {
          return;
        }
        let fileLower = file.toLowerCase();
        if (isCsvFile(file, true)) {
          if (!csvDelimiter && isCsvFile(file, false)) {
            return;
          }
          const x = Papa.parse<Record<string, any>>(await readTextFile(file), {
            header: true,
            skipEmptyLines: true,
            preview: maxRow,
            delimiter: realCsvDelimiter,
          });

          x.data.forEach((e, index) => {
            e[keyNotInTable] = index;
          });
          setPreViewData(x.data);
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
          setPreViewData(dataX);
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
          setPreViewData(data);
        } else if (isExcelFile(fileLower)) {
          const data = await commands.readExcelFilePreview(file, maxRow);
          setPreviewFields(data.fields);
          data.data.forEach((e, index) => {
            //@ts-ignore
            e[keyNotInTable] = index;
          });

          setPreViewData(data.data);
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
  }, [csvDelimiter, file, realCsvDelimiter, showNotification]);

  const diffColumns = useMemo(() => {
    if (!hasTableName) {
      return;
    }
    if (!file) {
      return;
    }

    let x = new Set(hasTableResp.fields.map((x) => x.field));
    let y = new Set(previewFields.map((x) => x.field));

    let r = { missing: [...setDifference(x, y)], unused: [...setDifference(y, x)] };
    if (r.missing.length == 0 && r.unused.length == 0) {
      return null;
    }
    return r;
  }, [file, hasTableName, hasTableResp.fields, previewFields]);

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
            <SqlTableImportFileNotice />
            <SelectPath path={file} setPath={setFile} required label="import file" />
            {file && fileLower.endsWith(".csv") && (
              <TextField
                value={csvDelimiter}
                onChange={(e) => {
                  setCsvDelimiter(e.target.value);
                }}
                label="csv delimiter"
              />
            )}
          </Stack>

          {file && !hasTableName && (
            <div style={{ flex: 1, height: "100px" }}>
              <SqlCreateTable
                fields={previewFields.map((e): CreateTableReqColumn => {
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
            </div>
          )}

          {hasTableName && (
            <Stack style={{ flex: 1, height: "100px" }}>
              <Typography color="info">
                <mark> {hasTableName}</mark> table data examples
              </Typography>

              <DataGrid
                style={{ flex: 1, height: "100px" }}
                columns={hasTableResp.fields}
                rows={hasTableResp.data}
                getRowId={(e) => e[keyNotInTable]}
              />
            </Stack>
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
            {file && (
              <Stack direction="row">
                <IconButton
                  loading={loading}
                  color="success"
                  disabled={!file}
                  onClick={async () => {
                    if (!tableName.current) {
                      showNotification({
                        severity: "error",
                        title: "table name is null,do u have create the table",
                      });
                      return;
                    }
                    if (fileLower.endsWith(".csv")) {
                      if (!csvDelimiter) {
                        showNotification({
                          severity: "error",
                          title: "csv deimiter not set",
                        });
                        return;
                      }
                      if (csvDelimiter.length != 1) {
                        showNotification({
                          severity: "error",
                          title: "csv delimiter should have one char",
                        });
                        return;
                      }
                    }
                    setLoading(true);
                    try {
                      await commands.tableImportFromFile(dbName, tableName.current, {
                        path: file,
                        mongo_data: null,
                        csv_delimiter: realCsvDelimiter,
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
                <Button
                  color="info"
                  onClick={async () => {
                    try {
                      let samples = await commands.tableImportDataFromFileInsertSamples(
                        dbName,
                        tableName.current,
                        {
                          path: file,
                          mongo_data: null,
                          csv_delimiter: realCsvDelimiter,
                        },
                      );
                      setInsertSqlSamples(samples);
                    } catch (e) {
                      showNotification({
                        severity: "error",
                        title: "try generate sql samples failed",
                        message: e,
                      });
                    }
                  }}>
                  sql samples
                </Button>
              </Stack>
            )}

            {diffColumns && (
              <>
                {diffColumns.missing.length > 0 ? (
                  <Typography color="warning">
                    Case conversion will not be attempted to match field names.
                  </Typography>
                ) : undefined}
                {diffColumns.missing.length ? (
                  <Stack
                    direction="row"
                    style={{ alignItems: "center", flexWrap: "wrap" }}>
                    <Typography color="waning"> missing columns:</Typography>
                    {diffColumns.missing.map((e) => (
                      <mark key={e}> {e} </mark>
                    ))}
                  </Stack>
                ) : undefined}

                {diffColumns.unused.length ? (
                  <Stack
                    direction="row"
                    style={{ alignItems: "center", flexWrap: "wrap" }}>
                    <Typography color="waning"> unused columns:</Typography>
                    {diffColumns.unused.map((e) => (
                      <mark key={e}> {e} </mark>
                    ))}
                  </Stack>
                ) : undefined}
              </>
            )}
            <DataGrid
              style={{ flex: 1, height: "100px" }}
              columns={previewFields}
              rows={previewData}
              getRowId={(e) => {
                return e[keyNotInTable];
              }}
            />

            {insertSqlSamples && (
              <Dialog
                open={true}
                maxWidth="md"
                style={{ overflow: "auto" }}
                onClose={() => {
                  setInsertSqlSamples(null);
                }}>
                <AppCode
                  language="sql"
                  codeBlock={(function () {
                    return insertSqlSamples.join(";\n");
                  })()}
                />
              </Dialog>
            )}
          </Stack>
        </Panel>
      )}
    </PanelGroup>
  );
}

function MongoImportFileNotice() {
  return (
    <LongNotice>
      <Typography>
        all data are treated as{" "}
        <mark
          onClick={() => {
            openUrl(
              "https://www.mongodb.com/ko-kr/docs/mongodb-shell/reference/ejson/",
            );
          }}
          style={{ cursor: "pointer" }}>
          EJSON
        </mark>
      </Typography>
      <AppCode
        language="json"
        extra={() => {
          return <> json examples</>;
        }}
        codeBlock={`[
{
  "_id": {
    "$oid": "69f2b6a5001ee3db63dccd30"
  },
  "name": "Alice",
  "age": 121
},
  ...
]`}
      />
      <AppCode
        language="jsonline"
        extra={() => {
          return <> jsonline examples</>;
        }}
        codeBlock={`{"_id": {"$oid": "69f2b6a5001ee3db63dccd30"},"name": "Alice","age": 121}
...`}
      />
    </LongNotice>
  );
}

function MongoTableImportFile(props: { hasTableName?: string }) {
  const { dbName } = useContext(DbnameContext);
  const { hasTableName } = props;
  const [file, setFile] = useState("");
  const { showNotification } = useNotification();
  const [data, setData] = useState<string[]>([]);
  const [tableName, setTableName] = useState(hasTableName);
  const [loading, setLoading] = useState(false);
  const fileLower = useMemo(() => {
    return file.toLowerCase();
  }, [file]);
  const maxRow = 10;

  useEffect(() => {
    (async () => {
      try {
        if (!file) {
          return;
        }

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
  }, [file, fileLower, showNotification]);

  const makeData = useCallback(async (): Promise<Uint8Array<ArrayBufferLike>[]> => {
    if (fileLower.endsWith(".jsonl")) {
      const ret = [];
      for await (const line of await readTextFileLines(file)) {
        ret.push(BSON.serialize(EJSON.parse(line)));
      }
      return ret;
    } else if (fileLower.endsWith(".json")) {
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
  }, [file, fileLower, showNotification]);

  return (
    <Stack
      style={{
        height: "100%",
        width: "100%",
        overflow: "auto",
      }}>
      <Stack direction="row" style={{ alignItems: "center" }}>
        <MongoImportFileNotice />
        <IconButton
          loading={loading}
          disabled={!file}
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
              await commands.tableImportFromFile(dbName, tableName, {
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

function CouchbaseImportFileNotice() {
  return (
    <LongNotice>
      <Typography>
        # The value nested inside &quot;value&quot; must be an object containing exactly
        2 fields
      </Typography>
      <Typography>
        # One of the fields must be &apos;id&apos;, and its value must be a string
      </Typography>
      <Typography>
        # The other field (non-&apos;id&apos;) is treated as the data value and must
        also be an object
      </Typography>
      <Typography>
        # The data from this example will be inserted into the collection
      </Typography>
      <Typography>
        # Example: id = &apos;aaaa&apos;, data = name: &apos;john&apos;, age: 18
      </Typography>

      <AppCode
        language="json"
        extra={() => {
          return <> json examples</>;
        }}
        codeBlock={`[

{
  "id": "aaaa",
  "test_show_table": {
    "name": "john",
    "age": 18
  }
},
...
]`}
      />
    </LongNotice>
  );
}

function CouchbaseTableImportFile(props: { hasTableName?: string }) {
  const { dbName } = useContext(DbnameContext);
  const { hasTableName } = props;
  const [file, setFile] = useState("");
  const { showNotification } = useNotification();
  const [data, setData] = useState<string[]>([]);
  const [tableName, setTableName] = useState(hasTableName);
  const [loading, setLoading] = useState(false);
  const fileLower = useMemo(() => {
    return file.toLowerCase();
  }, [file]);
  const maxRow = 10;

  useEffect(() => {
    (async () => {
      try {
        if (!file) {
          return;
        }

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
  }, [file, fileLower, showNotification]);

  return (
    <Stack
      style={{
        height: "100%",
        width: "100%",
        overflow: "auto",
      }}>
      <Stack direction="row" style={{ alignItems: "center" }}>
        <CouchbaseImportFileNotice />
        <IconButton
          loading={loading}
          disabled={!file}
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
              await commands.tableImportFromFile(dbName, tableName, {
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
  MongoTableImportFile,
  CouchbaseTableImportFile,
};
