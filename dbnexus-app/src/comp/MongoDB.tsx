import {
  Box,
  Button,
  ButtonGroup,
  Chip,
  Divider,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Pagination,
  Stack,
  Tooltip,
  Typography,
} from "@mui/material";

import * as monaco from "monaco-editor";

import ListIcon from "@mui/icons-material/List";
import { editor, languages, Position } from "monaco-editor";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import FilterAltIcon from "@mui/icons-material/FilterAlt";
import { FaArrowsAltV } from "react-icons/fa";
import {
  bsonValue2MongoBson,
  formatMongoBsonStr,
  MongoBsonBinaryAllSubType,
  MongoBsonError,
  parseMongoBson,
} from "./MongoBsonCompiler";
import { ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";

import { BSON, EJSON } from "bson";
import { TbJson } from "react-icons/tb";
import {
  DataGrid,
  GridCallbackDetails,
  GridPaginationModel,
  GridSlotsComponent,
  GridSlotsComponentsProps,
} from "@mui/x-data-grid";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import {
  modalStyle,
  reloadOK,
  keyNotInTable,
  EscDownEvent,
  modalHandleClose,
  FieldAndGridField,
} from "./Common";
import ArrowUpwardIcon from "@mui/icons-material/ArrowUpward";
import ArrowDownwardIcon from "@mui/icons-material/ArrowDownward";
import { BsFillLightbulbFill } from "react-icons/bs";
import DataObjectIcon from "@mui/icons-material/DataObject";
import {
  AppColumnMenu,
  ModalHeader,
  ResetColumnWidth,
  SelectPageSize,
} from "./CommonComp";
import {
  DocsAndCount,
  handleCopyToClipboard,
  ErrAndLoc,
  validJson,
  dataGridStyle,
  pageSizeOptions,
  shrinkAble,
  dataGridheaderFontSize,
  secondConfirm,
  MonacoEditor,
  extractFieldAndIDFromMouseEvent,
  DataSourceKind,
} from "./helper";
import {
  AppDataGridPagination,
  AppDataGridToolBar,
  EditorBar,
  ListenElementSize,
  SimpleFlexEditor,
  SliderSize,
  StackRowCenter,
} from "./CommonComp2";
import { message } from "@tauri-apps/plugin-dialog";
import {
  useColumnWidthStore,
  useContextMenu,
  useNotification,
  usePageSize,
  UseSizeConfig,
  useSizeStore,
  useTableColumnVisibleStore,
} from "./store";
import { commands, MuiDataGridData } from "./bindings";
import { AddTableModal } from "./LeftTable";
enum MongoDataViewModel {
  TableList,
  Object,
  Json,
}

const mongoID = "_id";

const mongoBsonKeywords =
  /\b(as|int|long|date|timestamp|decimal|binary|generic|function|byte_array|uuid|md5|encrypted|column|sensitive|int8_array|float32_array|user_define|true|false|null|undefined|code)\b/i;
function mongoBsonAutoCompletion(): languages.CompletionList {
  const generateSubTypes = MongoBsonBinaryAllSubType.allValues().map((e) => {
    return {
      label: e,
      insertText: e,
      kind: languages.CompletionItemKind.Operator,
    } as languages.CompletionItem;
  });

  return {
    suggestions: [
      ...generateSubTypes,
      {
        label: "as",
        kind: languages.CompletionItemKind.Keyword,
        insertText: "as",
        range: undefined,
      },
      {
        label: "int",
        kind: languages.CompletionItemKind.Class,
        insertText: "int",
        range: undefined,
      },
      {
        label: "long",
        kind: languages.CompletionItemKind.Class,
        insertText: "long",
        range: undefined,
      },
      {
        label: "date",
        kind: languages.CompletionItemKind.Class,
        insertText: "date",
        range: undefined,
      },
      {
        label: "timestamp",
        kind: languages.CompletionItemKind.Class,
        insertText: "timestamp",
        range: undefined,
      },
      {
        label: "decimal",
        kind: languages.CompletionItemKind.Class,
        insertText: "decimal",
        range: undefined,
      },
      {
        label: "binary",
        kind: languages.CompletionItemKind.Class,
        insertText: "binary",
        range: undefined,
      },
      {
        label: "ObjectID",
        kind: languages.CompletionItemKind.Class,
        insertText: "ObjectID",
        range: undefined,
      },
      {
        label: "code",
        kind: languages.CompletionItemKind.Class,
        insertText: "code",
        range: undefined,
      },
      {
        label: "true",
        kind: languages.CompletionItemKind.Constant,
        insertText: "true",
        range: undefined,
      },
      {
        label: "false",
        kind: languages.CompletionItemKind.Constant,
        insertText: "false",
        range: undefined,
      },

      {
        label: "float32_array",
        kind: languages.CompletionItemKind.Class,
        insertText: "float32",
        range: undefined,
      },
      {
        label: "int8_array",
        kind: languages.CompletionItemKind.Class,
        insertText: "int8",
        range: undefined,
      },
    ],
  };
}
const languageXXX = "mongoBson";

function initMongoBsonLanguage() {
  monaco.languages.register({ id: languageXXX });
  monaco.languages.setMonarchTokensProvider(languageXXX, {
    ignoreCase: true,
    tokenizer: {
      root: [
        [mongoBsonKeywords, "keyword"],
        [/"(?:\\.|[^\\"])*"/, "string"],
        [/'(?:\\.|[^\\'])*'/, "string"],
        [/`(?:\\.|[^\\`])*`/, "string"],
      ],
    },
    brackets: [
      { open: "{", close: "}", token: "delimiter.curly" },
      { open: "[", close: "]", token: "delimiter.parenthesis" },
    ],
  });

  monaco.languages.registerDocumentFormattingEditProvider(languageXXX, {
    provideDocumentFormattingEdits(model, options) {
      const text = model.getValue();
      try {
        const result = formatMongoBsonStr(text, options.tabSize);
        return [{ range: model.getFullModelRange(), text: result }];
      } catch (e) {
        if (e instanceof MongoBsonError) {
          message(e.msgWithLoc, { kind: "error" });
        } else {
          message(e.toString(), { kind: "error" });
        }
      }
    },
  });

  monaco.languages.registerCompletionItemProvider(languageXXX, {
    provideCompletionItems: () => {
      return mongoBsonAutoCompletion();
    },
  });
}

initMongoBsonLanguage();

function MongoBsonEditor(props: {
  doc: BSON.Document;
  setOP?: (doc: any) => Promise<void>;
  setOPName?;
  delOP?;
}) {
  let { doc, setOP, delOP, setOPName } = props;
  if (!setOPName) {
    setOPName = "set";
  }
  const editor = useRef<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position>(null);
  const [err, setErr] = useState<MongoBsonError>(null);
  const [loading, setLoading] = useState(false);
  const newDoc = useRef(doc);

  const handleEditorDidMount = useCallback((ce: editor.IStandaloneCodeEditor) => {
    editor.current = ce;
    ce.focus();
    const d1 = ce.onDidChangeModelContent(() => {
      const text = ce.getValue();
      try {
        const doc = parseMongoBson(text).compile();
        setErr(null);
        newDoc.current = doc;
      } catch (e) {
        setErr(e);
      }
    });
    const d2 = ce.onDidChangeCursorPosition((event) => {
      setPos(event.position);
    });
    return () => {
      d1.dispose();
      d2.dispose();
    };
  }, []);

  const { showNotification } = useNotification();
  const [content, setContent] = useState("{\n}");
  useEffect(() => {
    try {
      const c = bsonValue2MongoBson(doc).format();
      setContent(c);
    } catch (e) {
      showNotification({
        severity: "error",
        title: "format mongo value failed",
        message: e,
      });
    }
  }, [doc, showNotification]);
  return (
    <Stack style={{ flex: 1 }}>
      <div
        style={{
          flex: 1,
        }}>
        <MonacoEditor
          language={languageXXX}
          value={content}
          onMount={handleEditorDidMount}
        />
      </div>
      <EditorBar
        pos={pos}
        left={
          <StackRowCenter>
            {setOP && (
              <Button
                loading={loading}
                onClick={() => {
                  setLoading(true);
                  setOP(newDoc.current).finally(() => {
                    setLoading(false);
                  });
                }}
                disabled={!!err}>
                {setOPName}
              </Button>
            )}
            {delOP && (
              <Button
                variant="outlined"
                loading={loading}
                onClick={() => {
                  setLoading(true);
                  delOP().finally(() => {
                    setLoading(false);
                  });
                }}
                color="error">
                delete
              </Button>
            )}

            {err ? (
              <Typography
                color="error"
                style={{ cursor: "pointer" }}
                onClick={() => {
                  if (err.loc) {
                    editor.current.setPosition({
                      lineNumber: err.loc.start.line,
                      column: err.loc.end.column,
                    });
                    editor.current.focus();
                    editor.current.revealLines(err.loc.start.line, err.loc.end.line);
                  }
                }}>
                {err.msgWithLoc}
              </Typography>
            ) : (
              <></>
            )}
          </StackRowCenter>
        }
      />
    </Stack>
  );
}

let oldMongoValueEditorMode = MongoDataViewModel.Object;

function MongoValueEditor(props: {
  doc: BSON.Document;
  setOP?: (doc: BSON.Document) => Promise<void>;
  setName?;
  delOP?: () => Promise<void>;
  viewModel?: MongoDataViewModel;
  needConfirm: boolean;
}) {
  let { doc, setOP, setName, delOP, viewModel: viewmoDelSet, needConfirm } = props;
  if (!setName) {
    setName = "set";
  }
  const [viewModel, setViewModel] = useState(viewmoDelSet || oldMongoValueEditorMode);
  useEffect(() => {
    oldMongoValueEditorMode = viewModel;
  }, [viewModel]);
  const getViewModel = () => {
    return viewmoDelSet || viewModel;
  };
  const confirmAndSet = setOP
    ? async (doc) => {
        if (needConfirm) {
          const confirm = await secondConfirm({ title: "are u sure to modify" });
          if (!confirm) {
            return;
          }
        }
        setOP(doc);
      }
    : undefined;
  const confirmAndDel = delOP
    ? async () => {
        if (needConfirm) {
          const confirm = await secondConfirm({ title: "are u sure to delete" });
          if (!confirm) {
            return;
          }
        }
      }
    : undefined;
  return (
    <Stack style={{ flex: 1 }}>
      {!viewmoDelSet && (
        <Stack direction="row">
          <ButtonGroup>
            <Button
              disabled={viewModel == MongoDataViewModel.Object}
              onClick={() => {
                setViewModel(MongoDataViewModel.Object);
              }}>
              <DataObjectIcon />
            </Button>
            <Button
              onClick={() => {
                setViewModel(MongoDataViewModel.Json);
              }}
              disabled={viewModel == MongoDataViewModel.Json}>
              ejson
            </Button>
          </ButtonGroup>
        </Stack>
      )}
      {getViewModel() == MongoDataViewModel.Json && (
        <JsonEditor
          value={JSON.parse(EJSON.stringify(doc))}
          modifyOP={confirmAndSet}
          modifyOPName={setName}
          delOP={confirmAndDel}
        />
      )}
      {getViewModel() == MongoDataViewModel.Object && (
        <MongoBsonEditor
          doc={doc}
          setOP={confirmAndSet}
          setOPName={setName}
          delOP={confirmAndDel}
        />
      )}
    </Stack>
  );
}

function MongoTableToolBar(props: {
  handleReload: () => Promise<void>;
  handleFilter: (doc: BSON.Document) => void;
  handleSort: (doc: BSON.Document) => void;
  sort: BSON.Document;
  filter: BSON.Document;
  viewModel: MongoDataViewModel;
  handleViewModel: (doc: MongoDataViewModel) => void;
  someNotice?: ReactNode;
  loading: boolean;
  uuid: string;
  table: string;
  isTable: boolean;
  extra: ReactNode;
}) {
  const {
    handleReload,
    handleFilter,
    handleSort,
    sort,
    filter,
    viewModel,
    handleViewModel,
    someNotice,
    loading,
    uuid,
    table,
    isTable,
    extra,
  } = props;
  const [showModifyFilter, setShowModifyFilter] = useState(false);
  const [showModifySort, setShowModifySort] = useState(false);
  useEffect(() => {
    handleViewModel(viewModel);
  }, [handleViewModel, viewModel]);
  const [modalBox, setModalBox] = useState(null);

  return (
    <Stack
      direction="row"
      style={{
        justifyContent: "space-between",
        alignItems: "center",
      }}>
      <Stack direction="row" style={{ flexWrap: "wrap", alignItems: "center" }}>
        <Button loading={loading} color="primary" onClick={handleReload}>
          reload
        </Button>
        <Tooltip title={bsonValue2MongoBson(filter).format()}>
          <Chip
            label={
              oldMongoValueEditorMode == MongoDataViewModel.Object
                ? bsonValue2MongoBson(filter).format()
                : EJSON.stringify(filter)
            }
            onClick={() => {
              setShowModifyFilter(true);
            }}
            style={{ ...shrinkAble, maxWidth: "14vw" }}
            size="medium"
            color="primary"
            icon={<FilterAltIcon />}
            onDelete={
              Object.keys(filter).length
                ? () => {
                    handleFilter({});
                  }
                : undefined
            }
          />
        </Tooltip>
        <Tooltip title={bsonValue2MongoBson(sort).format()}>
          <Chip
            label={
              oldMongoValueEditorMode == MongoDataViewModel.Object
                ? bsonValue2MongoBson(sort).format()
                : EJSON.stringify(sort)
            }
            onClick={() => {
              setShowModifySort(true);
            }}
            style={{ ...shrinkAble, maxWidth: "14vw" }}
            size="medium"
            color="secondary"
            variant="outlined"
            icon={<FaArrowsAltV />}
            onDelete={
              Object.keys(sort).length
                ? () => {
                    handleSort({});
                  }
                : undefined
            }
          />
        </Tooltip>

        {isTable && (
          <AddTableModal uuid={uuid} kind={DataSourceKind.Mongo} tableName={table} />
        )}
        {someNotice}
      </Stack>
      {extra}

      <StackRowCenter>
        <ButtonGroup>
          <Button
            disabled={viewModel == MongoDataViewModel.Object}
            onClick={() => {
              handleViewModel(MongoDataViewModel.Object);
            }}>
            <DataObjectIcon />
          </Button>
          <Button
            onClick={() => {
              handleViewModel(MongoDataViewModel.Json);
            }}
            disabled={viewModel == MongoDataViewModel.Json}>
            ejson
          </Button>
          <Button
            onClick={() => {
              handleViewModel(MongoDataViewModel.TableList);
            }}
            disabled={viewModel == MongoDataViewModel.TableList}>
            <ListIcon />
          </Button>
        </ButtonGroup>
      </StackRowCenter>

      <ListenElementSize dom={modalBox} name="mongo-filter" set="both" />

      {showModifyFilter && (
        <Modal
          disableAutoFocus
          disableEnforceFocus
          open={showModifyFilter}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowModifyFilter(false);
          })}>
          <Box
            sx={{ resize: "both", width: "65%", height: "45vh", ...modalStyle }}
            ref={setModalBox}>
            <ModalHeader
              title="Modify Filter"
              handleClose={() => {
                setShowModifyFilter(false);
              }}
            />
            <MongoValueEditor
              setOP={async (doc) => {
                handleFilter(doc);
                setShowModifyFilter(false);
              }}
              doc={filter}
              needConfirm={false}
            />
          </Box>
        </Modal>
      )}

      {showModifySort && (
        <Modal
          disableAutoFocus
          disableEnforceFocus
          open={showModifySort}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowModifySort(false);
          })}>
          <Box
            sx={{ resize: "both", width: "65%", height: "45vh", ...modalStyle }}
            ref={setModalBox}>
            <ModalHeader
              handleClose={() => {
                setShowModifySort(false);
              }}
              title="Modify Sort"
            />
            <MongoValueEditor
              setOP={async (doc) => {
                handleSort(doc);
                setShowModifySort(false);
              }}
              needConfirm={false}
              doc={sort}
            />
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function MongoTableListContextMenu(props: {
  delOP?: () => void;
  content;
  field;
  setFilter;
  loadDoc: () => Promise<BSON.Document>;
  setSort;
}) {
  const { delOP, content, field, setFilter, loadDoc, setSort } = props;
  const [showFilter, setShowFilter] = useState(false);
  const { showNotification } = useNotification();
  const [showViewJson, setShowViewJson] = useState(false);
  const [modalBox, setModalBox] = useState(null);

  return (
    <div>
      <MenuList>
        {content && (
          <MenuItem
            onClick={() => {
              setShowViewJson(true);
            }}>
            <ListItemIcon>
              <TbJson fontSize="1.5rem" />
            </ListItemIcon>
            <ListItemText> View Json in Modal </ListItemText>
          </MenuItem>
        )}

        {content && (
          <MenuItem
            onClick={() => {
              handleCopyToClipboard(content, showNotification);
            }}>
            <ListItemIcon>
              <ContentCopyIcon />
            </ListItemIcon>
            <ListItemText> Copy </ListItemText>
          </MenuItem>
        )}
        <Divider />
        <MenuItem
          onClick={() => {
            const doc = {};
            doc[field] = 1;
            setSort(doc);
          }}>
          <ListItemIcon>
            <ArrowUpwardIcon />
          </ListItemIcon>
          <ListItemText> {field} ASC </ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            const doc = {};
            doc[field] = -1;
            setSort(doc);
          }}>
          <ListItemIcon>
            <ArrowDownwardIcon />
          </ListItemIcon>
          <ListItemText> {field} DESC </ListItemText>
        </MenuItem>
        <Divider />
        <MenuItem
          onClick={() => {
            setShowFilter(true);
          }}>
          <ListItemIcon>
            <FilterAltIcon />
          </ListItemIcon>
          <ListItemText> Quick Filter </ListItemText>
        </MenuItem>
        {delOP && (
          <MenuItem
            onClick={() => {
              delOP();
            }}>
            <ListItemIcon>
              <DeleteForeverIcon />
            </ListItemIcon>
            <ListItemText> Delete </ListItemText>
          </MenuItem>
        )}
      </MenuList>

      <ListenElementSize dom={modalBox} set="both" name="mongo-table-modify" />

      {showViewJson && (
        <Modal
          open={showViewJson}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowViewJson(false);
          })}>
          <Box
            sx={{ resize: "both", ...modalStyle, width: "75%", height: "65vh" }}
            ref={setModalBox}>
            <ModalHeader
              title="View Json"
              handleClose={() => {
                setShowViewJson(false);
              }}
            />
            <SimpleFlexEditor value={content} language="json" readOnly />
          </Box>
        </Modal>
      )}

      {showFilter && (
        <Modal
          open={showFilter}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowFilter(false);
          })}>
          <Box sx={{ ...modalStyle, width: "45%" }}>
            <ModalHeader
              title="Quick Filter"
              handleClose={() => {
                setShowFilter(false);
              }}
            />
            <MongoTableListMenuFilter
              loadValue={async () => {
                return loadDoc().then((x) => x[field]);
              }}
              setFilter={setFilter}
              field={field}
              handleClose={() => {
                setShowFilter(false);
              }}
            />
          </Box>
        </Modal>
      )}
    </div>
  );
}

let lastCompare = "$eq";
function MongoTableListMenuFilter(props: {
  handleClose;
  loadValue: () => Promise<any>;
  setFilter;
  field;
}) {
  const { handleClose, loadValue, setFilter, field } = props;
  const compares = ["$eq", "$ne", "$gt", "$gte", "$lt", "$lte"];
  const [compare, setCompare] = useState(lastCompare);
  const { showNotification } = useNotification();
  const [value, setValue] = useState(null);
  const [doc, setDoc] = useState({});
  useEffect(() => {
    loadValue()
      .then((v) => {
        setValue(v);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "get field value failed",
          message: e,
        });
      });
  }, [loadValue, showNotification]);
  useEffect(() => {
    lastCompare = compare;
  }, [compare]);
  useEffect(() => {
    const compareObj = {};
    if (compare == "starts") {
      compareObj[field] = { $regex: `^${value}`, $options: "i" };
      setDoc(compareObj);
      return;
    }
    if (compare == "ends") {
      compareObj[field] = { $regex: `${value}$`, $options: "i" };
      setDoc(compareObj);
      return;
    }
    if (compare == "contains") {
      compareObj[field] = { $regex: `${value}`, $options: "i" };
      setDoc(compareObj);
      return;
    }
    if (compare == "null") {
      compareObj[field] = null;
      setDoc(compareObj);
      return;
    }
    if (compare == "not null") {
      compareObj[field] = { $ne: null };
      setDoc(compareObj);
      return;
    }
    compareObj[compare] = value;
    const doc = {};
    doc[field] = compareObj;
    setDoc(doc);
  }, [compare, field, value]);

  return (
    <Stack>
      <Stack direction="row">
        {compares.map((x) => (
          <Chip
            key={x}
            onClick={() => {
              setCompare(x);
            }}
            label={x}
            color={compare === x ? "success" : "default"}
          />
        ))}
      </Stack>
      <StackRowCenter>
        {["null", "not null"].map((condition) => (
          <Chip
            key={condition}
            label={condition}
            color={compare === condition ? "success" : "default"}
            onClick={() => {
              setCompare(condition);
            }}
          />
        ))}
      </StackRowCenter>
      {typeof value == "string" && (
        <StackRowCenter>
          {["starts", "contains", "ends"].map((e) => (
            <Chip
              label={e}
              color={compare === e ? "success" : "default"}
              onClick={() => {
                setCompare(e);
              }}
              key={e}
            />
          ))}
        </StackRowCenter>
      )}

      <StackRowCenter>
        <Button
          onClick={() => {
            setFilter(doc);
            handleClose();
          }}>
          set
        </Button>
        <Typography color="success">{bsonValue2MongoBson(doc).format()}</Typography>
      </StackRowCenter>
    </Stack>
  );
}

function DoubleClickToModify() {
  return (
    <Tooltip title="double click a row to modify.">
      <BsFillLightbulbFill />
    </Tooltip>
  );
}
function JsonEditor(props: {
  value: object;
  modifyOP?: (x: object) => Promise<void>;
  modifyOPName?: string;
  delOP?: () => Promise<void>;
}) {
  const { value, modifyOP, delOP, modifyOPName } = props;
  const [newValue, setNewValue] = useState(EJSON.stringify(value, null, 2));
  const [jsonErr, setJsonErr] = useState<ErrAndLoc | null>(null);
  const [loading, setLoading] = useState(false);
  const [jsonErrPos, setJsonErrPos] = useState<Position>(null);
  const { showNotification } = useNotification();
  useEffect(() => {
    (async () => {
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
    })();
  }, [newValue, showNotification]);

  return (
    <SimpleFlexEditor
      readOnly={!modifyOP}
      value={newValue}
      language="json"
      errPos={jsonErrPos}
      handleValue={(v) => {
        setNewValue(v);
      }}
      barLeft=<Stack direction="row" style={{ alignItems: "center" }}>
        {modifyOP && (
          <Button
            disabled={!!jsonErr}
            onClick={() => {
              setLoading(true);
              modifyOP(EJSON.parse(newValue)).finally(() => {
                setLoading(false);
              });
            }}
            loading={loading}>
            {modifyOPName ? modifyOPName : "set"}
          </Button>
        )}
        {delOP && (
          <Button
            onClick={async () => {
              setLoading(true);
              delOP().finally(() => {
                setLoading(false);
              });
            }}
            loading={loading}
            variant="outlined"
            color="error">
            delete
          </Button>
        )}

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
    />
  );
}

function BsonJsonPagination({
  total,
  setPage,
  length,
  page,

  gridColCount,
  viewModel,
}) {
  const [showObjectDoc, setShowObjectDoc] = useState(false);
  const { mongo: pageSize, setPageSize } = usePageSize();
  return (
    <Stack
      direction="row"
      style={{ alignItems: "center", justifyContent: "space-between" }}>
      <Stack direction="row" style={{ alignItems: "center" }}>
        <Typography>
          result {length} of {total}
        </Typography>
        {viewModel == MongoDataViewModel.Object && (
          <Button
            variant="text"
            onClick={() => {
              setShowObjectDoc(true);
            }}>
            object?
          </Button>
        )}
      </Stack>

      <Stack direction="row" style={{ alignItems: "center" }}>
        <Pagination
          page={page}
          defaultPage={1}
          count={Math.ceil(total / pageSize)}
          onChange={(_e, page) => {
            setPage(page);
          }}
        />
        <SelectPageSize
          pageSize={pageSize}
          handleChange={(size) => {
            setPageSize("mongo", size);
          }}
          options={[2, 3, 5].map((x) => x * gridColCount)}
        />
      </Stack>

      {showObjectDoc && (
        <Modal
          open={showObjectDoc}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowObjectDoc(false);
          })}>
          <Box sx={{ ...modalStyle, width: "80vw", height: "80vh" }}>
            <ModalHeader
              title="mongo object"
              handleClose={() => {
                setShowObjectDoc(false);
              }}
            />
            <ModalObjectNotice />
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function ModalObjectNotice() {
  return (
    <Stack style={{ height: "100%" }}>
      <SimpleFlexEditor
        language={languageXXX}
        value={`{
  _id: '69bb51276b7795c05d1a25b2' as ObjectID, 
  string_field: 'Hello, world!',
  int32_field: 123,
  float_field: 123.45,
  bool_field: true,
  date_field: '2026-03-19T01:28:07.000Z' as date,
  null_field: null,
  regex_field: /haha/i,
  binary_field: [1,2] as binary int8_array,
  binary_field2: [1,2] as binary float32_array,
  binary_field3: '68656c6c6f' as binary generic,
  binary_field4: '68656c6c6f' as binary encrypted,
  binary_field5: '68656c6c6f' as binary sensitive,
  uuid_field: '0fe7263c138e480ea9e1d3869857dbca' as binary uuid,
  array_field: [1,2,3],
  object_field: {nested_key: 'nested_value'},
  decimal_field: '1' as decimal,
  timestamp_field: {t: 1773883687,i:1} as timestamp
}`}
      />
    </Stack>
  );
}

function MongoTableList(props: {
  reloadOP: (page: number, pageSize: number) => Promise<MuiDataGridData>;
  setSort;
  setFilter;
  loadOne: (id: any) => Promise<BSON.Document>;
  modifyOP?: (doc: BSON.Document) => Promise<void>;
  deleteOP?: (ids: any[] | any) => Promise<number>;
  triggerReload;
  loading: boolean;
  setLoading;
  tableName: string;
  uuid;
}) {
  const {
    reloadOP,
    setSort,
    loadOne,
    modifyOP,
    deleteOP,
    triggerReload,
    setFilter,
    loading,
    tableName,
    setLoading,
    uuid,
  } = props;
  const [page, setPage] = useState(0);
  const { mongo: pageSize, setPageSize } = usePageSize();
  const { showNotification } = useNotification();
  const [resp, setResp] = useState<MuiDataGridData | null>(null);
  const [selectModifyDoc, setSelectModifyDoc] = useState<BSON.Document | null>(null);
  const [showExport, setShowExport] = useState(false);
  const [clickedField, setClickedField] = useState("");
  const { getColumnWidth, setColumnWidth } = useColumnWidthStore();
  const handleReload = useCallback(
    async (showOK) => {
      setLoading(true);
      reloadOP(page + 1, pageSize)
        .then((r) => {
          r.data.forEach((e, index) => {
            e[keyNotInTable] = index + "";
          });
          r.fields.forEach((f) => {
            const width = getColumnWidth(f.field);
            if (width) {
              f["width"] = width;
            }
            f["renderHeader"] = (e) => {
              const field = e.colDef as unknown as FieldAndGridField;
              return (
                <Stack
                  onClick={(e) => {
                    e.stopPropagation();
                    setClickedField(field.field);
                  }}
                  style={{
                    alignItems: "center",
                    cursor: "default",
                  }}
                  direction="row">
                  <Typography sx={{ fontSize: dataGridheaderFontSize }}>
                    {field.field}
                  </Typography>
                </Stack>
              );
            };
          });
          setResp(r);
          if (showOK) {
            reloadOK(showNotification);
          }
        })
        .catch((e) => {
          showNotification({
            severity: "error",
            title: "fetch data failed",
            message: e,
          });
        })
        .finally(() => {
          setLoading(false);
        });
    },
    [getColumnWidth, page, pageSize, reloadOP, setLoading, showNotification],
  );

  useEffect(() => {
    handleReload(false);
  }, [handleReload, triggerReload]);

  const handleLoadOne = useCallback(
    async (id: any) => {
      try {
        return await loadOne(id);
      } catch (e) {
        showNotification({ severity: "error", title: "fetch data failed", message: e });
        return null;
      }
    },
    [loadOne, showNotification],
  );
  const getRowId = useCallback((e) => {
    return e[keyNotInTable];
  }, []);

  const onPaginationModelChange = useCallback(
    (e: GridPaginationModel, reason: GridCallbackDetails<"pagination">) => {
      setPage(e.page);
      if (reason.reason == "setPaginationModel") {
        setPageSize("mongo", e.pageSize);
      }
    },
    [setPageSize],
  );
  const onRowDoubleClick = useCallback(
    (e) => {
      handleLoadOne(EJSON.parse(e.row[mongoID])).then((doc) => {
        if (doc) {
          setSelectModifyDoc(doc);
        }
      });
    },
    [handleLoadOne],
  );

  const paginationModel = useMemo(() => {
    return { page, pageSize };
  }, [page, pageSize]);

  const onSortModelChange = useCallback(
    (e) => {
      if (e.length == 0) {
        setSort({});
        return;
      }
      const s = e[0];
      if (!s.sort) {
        setSort({});
        return;
      }
      const order = {};
      order[s.field] = s.sort == "asc" ? 1 : -1;
      setSort(order);
    },
    [setSort],
  );

  const handleDel = useMemo(() => {
    if (!resp) {
      return undefined;
    }
    return deleteOP
      ? async (id: string) => {
          if (!(await secondConfirm({ title: "sure to delete?" }))) {
            return;
          }
          const del = resp.data.find((e) => e[keyNotInTable] == id);
          try {
            await deleteOP([EJSON.parse(del[mongoID])]);
            showNotification({ severity: "success", title: "delete OK" });
            handleReload(false);
          } catch (e) {
            showNotification({ severity: "error", title: "delete failed", message: e });
          }
        }
      : undefined;
  }, [deleteOP, handleReload, resp, showNotification]);

  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const slotProps = useMemo(() => {
    if (!resp) {
      return undefined;
    }
    return {
      row: {
        onContextMenu: (e) => {
          const { field, id } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          const row = resp.data.find((e) => e[keyNotInTable] == id);
          setUpMenu({
            content: (
              <MongoTableListContextMenu
                content={row[field]}
                delOP={
                  handleDel
                    ? () => {
                        handleDel(id);
                      }
                    : undefined
                }
                field={field}
                setFilter={setFilter}
                loadDoc={async () => {
                  const doc = resp.data.find((x) => x[keyNotInTable] == id);
                  return await loadOne(EJSON.parse(doc[mongoID]));
                }}
                setSort={setSort}
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
    } as GridSlotsComponentsProps;
  }, [handleDel, loadOne, resp, setFilter, setSort, setUpMenu]);
  const slots = useMemo(() => {
    return {
      columnMenu: AppColumnMenu,
      toolbar: () => {
        return <AppDataGridToolBar title={tableName} />;
      },
      pagination: () => {
        return (
          <Stack
            direction="row"
            style={{
              alignItems: "center",
              justifyContent: "space-between",
              overflow: "auto",
              flex: 1,
            }}>
            <Stack style={{ flex: 1, alignItems: "center" }} direction="row">
              <ResetColumnWidth />
              <DoubleClickToModify />
            </Stack>
            <AppDataGridPagination showPageSelect />
          </Stack>
        );
      },
    } as Partial<GridSlotsComponent>;
  }, [tableName]);

  const onColumnResize = useCallback(
    ({ colDef, width }) => {
      setColumnWidth(colDef.field, width);
    },
    [setColumnWidth],
  );
  const [modalBox, setModalBox] = useState(null);
  const { getColumnVisibility, setColumnVisibility } = useTableColumnVisibleStore(
    (s) => s,
  );
  return (
    <Stack style={{ height: "100%" }}>
      {resp && (
        <DataGrid
          getCellClassName={(e) => {
            if (e.field == clickedField) {
              return "datagrid-field-clicked";
            } else {
              return "";
            }
          }}
          onCellClick={() => {
            setClickedField("");
          }}
          columnVisibilityModel={getColumnVisibility(uuid, tableName)}
          onColumnVisibilityModelChange={(e) => {
            setColumnVisibility(uuid, tableName, e);
          }}
          loading={loading}
          slots={slots}
          style={dataGridStyle}
          slotProps={slotProps}
          getRowId={getRowId}
          rowCount={resp.total}
          columns={resp.fields}
          rows={resp.data}
          pageSizeOptions={pageSizeOptions}
          paginationMode="server"
          paginationModel={paginationModel}
          onPaginationModelChange={onPaginationModelChange}
          onRowDoubleClick={onRowDoubleClick}
          sortingMode="server"
          onSortModelChange={onSortModelChange}
          onColumnResize={onColumnResize}
        />
      )}

      <ListenElementSize dom={modalBox} set="both" name="mongo-table-modify-doc" />

      {selectModifyDoc && (
        <Modal
          disableAutoFocus
          disableEnforceFocus
          open={!!selectModifyDoc}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setSelectModifyDoc(null);
          })}>
          <Box
            sx={{ resize: "both", ...modalStyle, width: "75%", height: "60vh" }}
            ref={setModalBox}>
            <ModalHeader
              title="Modify data"
              handleClose={() => {
                setSelectModifyDoc(null);
              }}
            />
            <MongoValueEditor
              setOP={
                modifyOP
                  ? async (doc) => {
                      try {
                        await modifyOP(doc);
                      } catch (e) {
                        showNotification({
                          severity: "error",
                          title: "Modify failed",
                          message: e,
                        });
                        return;
                      }
                      showNotification({ severity: "success", title: "Modify OK" });
                      setSelectModifyDoc(null);
                      handleReload(false);
                    }
                  : undefined
              }
              needConfirm
              doc={selectModifyDoc}
            />
          </Box>
        </Modal>
      )}

      {showExport && (
        <Modal
          open={showExport}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowExport(false);
          })}>
          <Box sx={{ ...modalStyle, width: "60%" }}>
            <ModalHeader
              title="export markdown table"
              handleClose={() => {
                setShowExport(false);
              }}
            />
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function renderId(x) {
  return EJSON.stringify(x);
}
function MongoTableObject(props: {
  reloadOPDocs: (page: number, pageSize: number) => Promise<DocsAndCount>;
  modifyOP?: (doc: BSON.Document) => Promise<void>;
  deleteOP?: (ids: any[] | any) => Promise<number>;
  triggerReload;
  setLoading;
  editorHeight: number;
  gridColCount: number;
  viewModel: MongoDataViewModel;
}) {
  const {
    reloadOPDocs,
    modifyOP,
    deleteOP,
    triggerReload,
    setLoading,
    editorHeight,
    gridColCount,
    viewModel,
  } = props;
  const [docs, setDocs] = useState<DocsAndCount>({ docs: [], count: 0 });
  const [page, setPage] = useState(1);
  const { mongo: pageSize } = usePageSize();

  const { showNotification } = useNotification();
  const handleReload = useCallback(
    async (showOK) => {
      setLoading(true);
      reloadOPDocs(page, pageSize)
        .then((resp) => {
          setDocs(resp);
          if (showOK) {
            reloadOK(showNotification);
          }
        })
        .catch((e) => {
          showNotification({
            severity: "error",
            title: "fetch data failed",
            message: e,
          });
        })
        .finally(() => {
          setLoading(false);
        });
    },
    [page, pageSize, reloadOPDocs, setLoading, showNotification],
  );

  const handleDelete = async (ids: any[]) => {
    try {
      await deleteOP(ids);
    } catch (e) {
      showNotification({ severity: "error", title: "Delete failure", message: e });
    }
    showNotification({ severity: "success", title: "Delete OK" });
    handleReload(false);
  };

  useEffect(() => {
    handleReload(false);
  }, [handleReload]);

  useEffect(() => {
    handleReload(true);
  }, [handleReload, triggerReload]);
  return (
    <Stack
      style={{
        flexShrink: 1,
        flexGrow: 1,
        flexBasis: "0",
        height: "100px",
      }}>
      <div
        style={{
          flex: 1,
          height: "100px",
          overflowY: "auto",
          overflowX: "scroll",
          display: "grid",
          gridTemplateColumns: `repeat(${gridColCount}, 1fr)`,
          paddingRight: "1.2rem",
        }}>
        {docs.docs.map((doc, index) => {
          return (
            <div
              key={renderId(doc[mongoID])}
              style={{
                height: editorHeight + "px",
                display: "flex",
                position: "relative",
              }}>
              <Typography
                color="info"
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  zIndex: 14,
                  opacity: 0.5,
                }}>
                #{index + 1}
              </Typography>
              <MongoValueEditor
                doc={doc}
                viewModel={viewModel}
                setOP={
                  modifyOP
                    ? async (doc) => {
                        try {
                          await modifyOP(doc);
                        } catch (e) {
                          showNotification({
                            severity: "error",
                            title: "Modify failed",
                            message: e,
                          });
                          return;
                        }
                        showNotification({ severity: "success", title: "modify ok" });
                        handleReload(false);
                      }
                    : undefined
                }
                delOP={
                  deleteOP
                    ? async () => {
                        return await handleDelete(doc[mongoID]);
                      }
                    : undefined
                }
                needConfirm
              />
            </div>
          );
        })}
      </div>

      {/* // here is pagination */}
      <BsonJsonPagination
        viewModel={viewModel}
        total={docs.count}
        length={docs.docs.length}
        page={page}
        setPage={setPage}
        gridColCount={gridColCount}
      />
    </Stack>
  );
}

type MongoShowTablePara = {
  filter: Uint8Array;
  page: number;
  page_size: number;
  sort: Uint8Array;
};

class MongoTableHandler {
  static makeShowTablePara(filter, sort, page, pageSize): MongoShowTablePara {
    return {
      filter: BSON.serialize(filter),
      sort: BSON.serialize(sort),
      page,
      page_size: pageSize,
    };
  }

  static async reloadTableList(
    uuid: string,
    table: string,
    filter,
    sort,
    page,
    pageSize,
  ) {
    let para: MongoShowTablePara = MongoTableHandler.makeShowTablePara(
      filter,
      sort,
      page,
      pageSize,
    );
    return await commands.mongoShowCollectionTableList(uuid, table, para);
  }

  static reloadDocs(uuid: string, table: string, filter, sort, page, pageSize) {
    let para: MongoShowTablePara = MongoTableHandler.makeShowTablePara(
      filter,
      sort,
      page,
      pageSize,
    );
    return commands.mongoShowCollectionDocs(uuid, table, para).then((x) => {
      const docs = x.docs.map((e) => BSON.deserialize(new Uint8Array(e)));
      const count = x.count;
      return new DocsAndCount(docs, count);
    });
  }

  static loadOne(uuid: string, table: string) {
    return async (id): Promise<BSON.Document> => {
      return commands
        .mongoGetOneDocumentById(uuid, table, JSON.stringify(EJSON.serialize(id)))
        .then((x) => {
          return BSON.deserialize(new Uint8Array(x));
        });
    };
  }

  static modifyOP(uuid: string, table: string) {
    return async (doc) => {
      return commands.mongoUpdateOne(uuid, table, BSON.serialize(doc));
    };
  }
  static deleteOP(uuid, table) {
    return async (ids) => {
      if (!Array.isArray(ids)) {
        ids = [ids];
      }
      return commands.mongoDeleteIds(uuid, table, JSON.stringify(EJSON.serialize(ids)));
    };
  }
}
let oldViewModel = MongoDataViewModel.Object;
function MongoTable(props: { uuid: string; table: string; isTable: boolean }) {
  const { uuid, table, isTable } = props;
  const modifyOP = isTable ? MongoTableHandler.modifyOP(uuid, table) : undefined;
  const loadOne = MongoTableHandler.loadOne(uuid, table);
  const deleteOP = isTable ? MongoTableHandler.deleteOP(uuid, table) : undefined;
  const [filter, setFilter] = useState<BSON.Document>({});
  const [sort, setSort] = useState<BSON.Document>({});
  const [viewModel, setViewModel] = useState(oldViewModel);
  const [triggerReload, setTriggerReload] = useState(0);
  const [loading, setLoading] = useState(false);
  useEffect(() => {
    oldViewModel = viewModel;
  }, [viewModel]);

  const { getSize } = useSizeStore();

  const reloadTable = useCallback(
    (page: number, pageSize: number) => {
      return MongoTableHandler.reloadTableList(
        uuid,
        table,
        filter,
        sort,
        page,
        pageSize,
      );
    },
    [filter, sort, table, uuid],
  );
  useEffect(() => {}, [reloadTable]);
  const reloadDocs = useCallback(
    (page: number, pageSize: number) => {
      return MongoTableHandler.reloadDocs(uuid, table, filter, sort, page, pageSize);
    },
    [filter, sort, table, uuid],
  );

  return (
    <Stack style={{ height: "100%" }}>
      <MongoTableToolBar
        sort={sort}
        filter={filter}
        handleFilter={(e) => {
          setFilter(e);
        }}
        handleSort={(e) => {
          setSort(e);
        }}
        handleReload={async () => {
          setTriggerReload((prev) => prev + 1);
        }}
        handleViewModel={(e) => {
          setViewModel(e);
        }}
        viewModel={viewModel}
        loading={loading}
        uuid={uuid}
        table={table}
        isTable={isTable}
        extra={
          viewModel != MongoDataViewModel.TableList && (
            <Stack direction="row">
              <SliderSize config={UseSizeConfig.gridCol} />
              <SliderSize config={UseSizeConfig.editorHeight} />
            </Stack>
          )
        }
      />
      {viewModel == MongoDataViewModel.TableList && (
        <MongoTableList
          setFilter={setFilter}
          reloadOP={reloadTable}
          loadOne={loadOne}
          modifyOP={modifyOP}
          setSort={setSort}
          deleteOP={deleteOP}
          triggerReload={triggerReload}
          loading={loading}
          setLoading={setLoading}
          tableName={table}
          uuid={uuid}
        />
      )}
      {viewModel != MongoDataViewModel.TableList && (
        <MongoTableObject
          reloadOPDocs={reloadDocs}
          modifyOP={modifyOP}
          deleteOP={deleteOP}
          triggerReload={triggerReload}
          setLoading={setLoading}
          gridColCount={getSize(UseSizeConfig.gridCol)}
          editorHeight={getSize(UseSizeConfig.editorHeight)}
          viewModel={viewModel}
        />
      )}
    </Stack>
  );
}

function MongoTableAdd(props: { addOP: (doc: BSON.Document) => Promise<string> }) {
  const { addOP } = props;
  const { showNotification } = useNotification();
  const handleAdd = async (doc) => {
    try {
      await addOP(doc);
    } catch (e) {
      showNotification({ severity: "error", title: "Add failure", message: e });
      return;
    }
    showNotification({ severity: "success", title: "Add OK" });
  };

  return (
    <Stack
      style={{
        height: "100%",
      }}>
      <MongoValueEditor doc={{}} setName="add" setOP={handleAdd} needConfirm={false} />
    </Stack>
  );
}

export { MongoValueEditor, MongoTable, MongoTableAdd };
