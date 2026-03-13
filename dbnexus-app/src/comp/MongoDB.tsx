import Editor from "@monaco-editor/react";
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
import {
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { loader } from "./helper";
import { BSON, EJSON } from "bson";
import { NotificationContext } from "./Notification";
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
  TimeDuration,
} from "./CommonComp";
import {
  ColumnWidth,
  ContextMenuContext,
  DocsAndCount,
  handleCopyToClipboard,
  ErrAndLoc,
  saveAndSetSize,
  SettingContext,
  validJson,
  SavePageSize,
  MuiDataGridData,
  dataGridStyle,
  pageSizeOptions,
  shrinkAble,
  DataSourceKind,
  dataGridheaderFontSize,
  secondConfirm,
} from "./helper";
import {
  AppDataGridPagination,
  AppDataGridToolBar,
  EditorBar,
  LastFetchTime,
  SimpleFlexEditor,
  SliderSize,
  StackRowCenter,
  TableAddButton,
} from "./CommonComp2";
import { message } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { AddTableView } from "./LeftTable";
enum MongoDataViewModel {
  List,
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
  loader
    .init()
    .then((monaco) => {
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
    })
    .catch((e) => {
      console.error("init monaco language error:", e);
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
  const setting = useContext(SettingContext);
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

  const { showNotification } = useContext(NotificationContext);
  const [content, setContent] = useState("{\n}");
  useEffect(() => {
    try {
      const c = bsonValue2MongoBson(doc).format();
      setContent(c);
    } catch (e) {
      showNotification("error", "format mongo value failed", e);
    }
  }, [doc, showNotification]);
  const options = useMemo((): editor.IStandaloneEditorConstructionOptions => {
    const { fontSize, fontFamily, fontWeight } = setting.appearance;
    return {
      readOnly: !setOP,
      fontSize,
      fontWeight,
      fontFamily,
      tabSize: setting.editor.tabSize,
      ...setting.editor.editorOption,
    };
  }, [setOP, setting.appearance, setting.editor.editorOption, setting.editor.tabSize]);
  return (
    <Stack flex={1}>
      <div
        style={{
          flex: 1,
        }}
      >
        <Editor
          width="99%"
          height="99%"
          onMount={handleEditorDidMount}
          language={languageXXX}
          value={content}
          options={options}
          theme={setting.editorTheme()}
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
                disabled={!!err}
              >
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
                color="error"
              >
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
                }}
              >
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
    <Stack flex={1}>
      {!viewmoDelSet && (
        <Stack direction="row">
          <ButtonGroup>
            <Button
              disabled={viewModel == MongoDataViewModel.Object}
              onClick={() => {
                setViewModel(MongoDataViewModel.Object);
              }}
            >
              <DataObjectIcon />
            </Button>
            <Button
              onClick={() => {
                setViewModel(MongoDataViewModel.Json);
              }}
              disabled={viewModel == MongoDataViewModel.Json}
            >
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
  const [showAdd, setShowAdd] = useState(false);
  useEffect(() => {
    handleViewModel(viewModel);
  }, [handleViewModel, viewModel]);
  const [modalBox, setModalBox] = useState(null);
  useEffect(() => {
    if (modalBox) {
      const c = saveAndSetSize(modalBox, "mongo-filter-sort");
      return c;
    }
  }, [modalBox]);

  const [addModalBox, setAddModalBox] = useState(null);
  useEffect(() => {
    if (addModalBox) {
      const x1 = saveAndSetSize(addModalBox, "table-insert");
      return () => {
        x1();
      };
    }
  }, [addModalBox]);

  return (
    <Stack
      direction="row"
      style={{
        justifyContent: "space-between",
        alignItems: "center",
      }}
    >
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
          />
        </Tooltip>

        {isTable && (
          <TableAddButton
            handleAdd={() => {
              setShowAdd(true);
            }}
          />
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
            }}
          >
            <DataObjectIcon />
          </Button>
          <Button
            onClick={() => {
              handleViewModel(MongoDataViewModel.Json);
            }}
            disabled={viewModel == MongoDataViewModel.Json}
          >
            ejson
          </Button>
          <Button
            onClick={() => {
              handleViewModel(MongoDataViewModel.List);
            }}
            disabled={viewModel == MongoDataViewModel.List}
          >
            <ListIcon />
          </Button>
        </ButtonGroup>
      </StackRowCenter>

      {showModifyFilter && (
        <Modal
          open={showModifyFilter}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowModifyFilter(false);
          })}
        >
          <Box
            sx={{ resize: "both", ...modalStyle }}
            width="65%"
            ref={setModalBox}
            height="45vh"
          >
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
          open={showModifySort}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowModifySort(false);
          })}
        >
          <Box
            sx={{ resize: "both", ...modalStyle }}
            width="65%"
            ref={setModalBox}
            height="45vh"
          >
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
            <AddTableView uuid={uuid} table={table} kind={DataSourceKind.Mongo} />
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function renderId(_id: any): string {
  // console.log("_id", _id, EJSON.stringify(_id));
  return EJSON.stringify(_id);
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
  const { showNotification } = useContext(NotificationContext);
  const [showViewJson, setShowViewJson] = useState(false);
  const [modalBox, setModalBox] = useState(null);

  useEffect(() => {
    if (modalBox) {
      const c = saveAndSetSize(modalBox, "mongo--list-view-json");
      return c;
    }
  }, [modalBox]);

  return (
    <div>
      <MenuList>
        {content && (
          <MenuItem
            onClick={() => {
              setShowViewJson(true);
            }}
          >
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
            }}
          >
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
          }}
        >
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
          }}
        >
          <ListItemIcon>
            <ArrowDownwardIcon />
          </ListItemIcon>
          <ListItemText> {field} DESC </ListItemText>
        </MenuItem>
        <Divider />
        <MenuItem
          onClick={() => {
            setShowFilter(true);
          }}
        >
          <ListItemIcon>
            <FilterAltIcon />
          </ListItemIcon>
          <ListItemText> Quick Filter </ListItemText>
        </MenuItem>
        {delOP && (
          <MenuItem
            onClick={() => {
              delOP();
            }}
          >
            <ListItemIcon>
              <DeleteForeverIcon />
            </ListItemIcon>
            <ListItemText> Delete </ListItemText>
          </MenuItem>
        )}
      </MenuList>

      {showViewJson && (
        <Modal
          open={showViewJson}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowViewJson(false);
          })}
        >
          <Box
            sx={{ resize: "both", ...modalStyle }}
            ref={setModalBox}
            width="75%"
            height="65vh"
          >
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
          })}
        >
          <Box sx={modalStyle} width="45%">
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
  const { showNotification } = useContext(NotificationContext);
  const [value, setValue] = useState(null);
  const [doc, setDoc] = useState({});
  useEffect(() => {
    loadValue()
      .then((v) => {
        setValue(v);
      })
      .catch((e) => {
        showNotification("error", "get field value failed", e);
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
          }}
        >
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
  const { showNotification } = useContext(NotificationContext);
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

  return (
    <SimpleFlexEditor
      readOnly={!modifyOP}
      value={newValue}
      language="json"
      errPos={jsonErrPos}
      handleValue={(v) => {
        setNewValue(v);
      }}
      barLeft=<Stack direction="row" alignItems="center">
        {modifyOP && (
          <Button
            disabled={!!jsonErr}
            onClick={() => {
              setLoading(true);
              modifyOP(EJSON.parse(newValue)).finally(() => {
                setLoading(false);
              });
            }}
            loading={loading}
          >
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
            color="error"
          >
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
            }}
          >
            {jsonErr.msg}
          </Typography>
        )}
      </Stack>
    />
  );
}

function BsonJsonPagination({
  pageSize,
  total,
  setPage,
  length,
  page,
  setPageSize,
  gridColCount,
}) {
  return (
    <Stack direction="row" style={{ alignItems: "center", justifyContent: "flex-end" }}>
      <Typography>
        result {length} of {total}
      </Typography>
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
          setPageSize(size);
        }}
        options={[2, 3, 5].map((x) => x * gridColCount)}
      />
    </Stack>
  );
}

function MongoTableList(props: {
  filter: BSON.Document;
  sort: BSON.Document;
  reloadOP: (
    filter: BSON.Document,
    sort: BSON.Document,
    page: number,
    pageSize: number,
  ) => Promise<MuiDataGridData>;
  setSort;
  setFilter;
  loadOne: (id: any) => Promise<BSON.Document>;
  modifyOP?: (doc: BSON.Document) => Promise<void>;
  deleteOP?: (ids: any[] | any) => Promise<void>;
  triggerReload;
  loading: boolean;
  setLoading;
  tableName: string;
}) {
  const {
    filter,
    sort,
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
  } = props;
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(SavePageSize.instance.get());
  const { showNotification } = useContext(NotificationContext);
  const [resp, setResp] = useState<MuiDataGridData | null>(null);
  const [selectModifyDoc, setSelectModifyDoc] = useState<BSON.Document | null>(null);
  const [showExport, setShowExport] = useState(false);
  const setting = useContext(SettingContext);
  const [lastFetchTime, setLastFetchTime] = useState(new Date());
  const [clickedField, setClickedField] = useState("");
  useEffect(() => {
    setLastFetchTime(new Date());
  }, [resp]);
  const [fetchDur, setFetchDur] = useState(0);
  const handleReload = useCallback(
    async (showOK) => {
      const now = new Date();
      setLoading(true);
      reloadOP(filter, sort, page + 1, pageSize)
        .then((r) => {
          r.data.forEach((e, index) => {
            e[keyNotInTable] = index;
          });
          r.fields.forEach((f) => {
            const width = ColumnWidth.instance.get(f.field);
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
                  direction="row"
                >
                  <Typography fontSize={dataGridheaderFontSize}>
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
          showNotification("error", "fetch data failed", e);
        })
        .finally(() => {
          setFetchDur(new Date().getTime() - now.getTime());
          setLoading(false);
        });
    },
    [filter, page, pageSize, reloadOP, setLoading, showNotification, sort],
  );

  useEffect(() => {
    handleReload(false);
  }, [page, pageSize, filter, sort, handleReload]);

  useEffect(() => {
    handleReload(true);
  }, [handleReload, triggerReload]);

  const handleLoadOne = useCallback(
    async (id: any) => {
      try {
        return await loadOne(id);
      } catch (e) {
        showNotification("error", "fetch data failed", e);
        return null;
      }
    },
    [loadOne, showNotification],
  );
  const getRowId = useCallback((e) => {
    return renderId(e[keyNotInTable]);
  }, []);
  const onPaginationModelChange = useCallback(
    (e: GridPaginationModel, reason: GridCallbackDetails<"pagination">) => {
      setPage(e.page);
      setPageSize(e.pageSize);
      if (reason.reason == "setPaginationModel") {
        SavePageSize.instance.save(e.pageSize);
      }
    },
    [],
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
            showNotification("success", "delete OK");
            handleReload(false);
          } catch (e) {
            showNotification("error", "delete failed", e);
          }
        }
      : undefined;
  }, [deleteOP, handleReload, resp, showNotification]);

  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    if (!resp) {
      return undefined;
    }
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          const field = (target as HTMLDivElement).getAttribute("data-field");
          const id = (currentTarget as HTMLDivElement).getAttribute("data-id");
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
      toolbar: {
        csvOptions: {
          delimiter: setting.export.csvDelimiter,
          includeHeaders: setting.export.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [
    handleDel,
    loadOne,
    resp,
    setFilter,
    setSort,
    setUpMenu,
    setting.export.csvDelimiter,
    setting.export.includeHeader,
  ]);
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
            flex={1}
            style={{
              alignItems: "center",
              justifyContent: "space-between",
              overflow: "auto",
            }}
          >
            <Stack style={{ flex: 1, alignItems: "center" }} direction="row">
              <LastFetchTime date={lastFetchTime} />
              <TimeDuration dur={fetchDur} />
              <ResetColumnWidth />
              <DoubleClickToModify />
            </Stack>
            <AppDataGridPagination showPageSelect />
          </Stack>
        );
      },
    } as Partial<GridSlotsComponent>;
  }, [fetchDur, lastFetchTime, tableName]);
  const onColumnResize = useCallback(({ colDef, width }) => {
    ColumnWidth.instance.save(colDef.field, width);
  }, []);
  const [modalBox, setModalBox] = useState(null);
  useEffect(() => {
    if (modalBox) {
      const clean = saveAndSetSize(modalBox, "mongo-list-modify");
      return clean;
    }
  }, [modalBox]);
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
      {selectModifyDoc && (
        <Modal
          open={!!selectModifyDoc}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setSelectModifyDoc(null);
          })}
          disableAutoFocus
          disableEnforceFocus
        >
          <Box
            sx={{ resize: "both", ...modalStyle }}
            width="75%"
            height="60vh"
            ref={setModalBox}
          >
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
                        showNotification("error", "Modify failed", e);
                        return;
                      }
                      showNotification("success", "Modify OK");
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
          })}
        >
          <Box sx={modalStyle} width="60%">
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

function MongoTableObject(props: {
  filter: BSON.Document;
  sort: BSON.Document;
  reloadOPDocs: (
    filter: BSON.Document,
    sort: BSON.Document,
    page: number,
    pageSize: number,
  ) => Promise<DocsAndCount>;
  modifyOP?: (doc: BSON.Document) => Promise<void>;
  deleteOP?: (ids: any[] | any) => Promise<void>;
  triggerReload;
  setLoading;
  editorHeight: number;
  gridColCount: number;
  viewModel: MongoDataViewModel;
}) {
  const {
    filter,
    sort,
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
  const [pageSize, setPageSize] = useState(SavePageSize.mongodb.get());
  useEffect(() => {
    SavePageSize.mongodb.save(pageSize);
  }, [pageSize]);

  const { showNotification } = useContext(NotificationContext);
  const handleReload = useCallback(
    async (showOK) => {
      setLoading(true);
      reloadOPDocs(filter, sort, page, pageSize)
        .then((resp) => {
          setDocs(resp);
          if (showOK) {
            reloadOK(showNotification);
          }
        })
        .catch((e) => {
          showNotification("error", "fetch data failed", e);
        })
        .finally(() => {
          setLoading(false);
        });
    },
    [filter, page, pageSize, reloadOPDocs, setLoading, showNotification, sort],
  );

  const handleDelete = async (ids: any[]) => {
    try {
      await deleteOP(ids);
    } catch (e) {
      showNotification("error", "Delete failure", e);
    }
    showNotification("success", "Delete OK");
    handleReload(false);
  };
  useEffect(() => {
    handleReload(false);
  }, [page, pageSize, filter, sort, handleReload]);

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
      }}
    >
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
        {docs.docs.map((doc, index) => {
          return (
            <div
              key={renderId(doc[mongoID])}
              style={{
                height: editorHeight + "px",
                display: "flex",
                position: "relative",
              }}
            >
              <Typography
                color="info"
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  zIndex: 14,
                  opacity: 0.5,
                }}
              >
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
                          showNotification("error", "Modify failed", e);
                          return;
                        }
                        showNotification("success", "modify ok");
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
        total={docs.count}
        length={docs.docs.length}
        page={page}
        setPage={setPage}
        pageSize={pageSize}
        setPageSize={setPageSize}
        gridColCount={gridColCount}
      />
    </Stack>
  );
}

class MongoTableHandler {
  static reloadTableList(uuid: string, table: string) {
    return async (filter, sort, page, pageSize): Promise<MuiDataGridData> => {
      return await invoke("mongo_show_collection_table_list", {
        name: uuid,
        tableName: table,
        filter: BSON.serialize(filter),
        sort: BSON.serialize(sort),
        page,
        pageSize,
      });
    };
  }
  static reloadDocs(uuid: string, table: string) {
    return async (filter, sort, page, pageSize): Promise<DocsAndCount> => {
      return invoke<{ count: number; docs: number[][] }>("mongo_show_collection_docs", {
        name: uuid,
        tableName: table,
        filter: BSON.serialize(filter),
        sort: BSON.serialize(sort),
        page,
        pageSize,
      }).then((x) => {
        const docs = x.docs.map((e) => BSON.deserialize(new Uint8Array(e)));
        const count = x.count;
        return new DocsAndCount(docs, count);
      });
    };
  }

  static loadOne(uuid: string, table: string) {
    return async (id): Promise<BSON.Document> => {
      return invoke<number[]>("mongo_get_one_document_by_id", {
        name: uuid,
        tableName: table,
        id: JSON.stringify(EJSON.serialize(id)),
      }).then((x) => {
        return BSON.deserialize(new Uint8Array(x));
      });
    };
  }

  static modifyOP(uuid: string, table: string) {
    return async (doc) => {
      return invoke<void>("mongo_update_one", {
        name: uuid,
        tableName: table,
        doc: BSON.serialize(doc),
      });
    };
  }
  static deleteOP(uuid, table) {
    return async (ids) => {
      if (!Array.isArray(ids)) {
        ids = [ids];
      }
      return invoke<void>("mongo_delete_ids", {
        name: uuid,
        tableName: table,
        ids: JSON.stringify(EJSON.serialize(ids)),
      });
    };
  }
}
let oldViewModel = MongoDataViewModel.Object;
function MongoTable(props: { uuid: string; table: string; isTable: boolean }) {
  const { uuid, table, isTable } = props;
  const reloadOPTableList = MongoTableHandler.reloadTableList(uuid, table);
  const reloadOPDocs = MongoTableHandler.reloadDocs(uuid, table);
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
  const [gridColCount, setGridColCount] = useState(3);
  const [editorHeight, setEditorHeight] = useState(500);
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
          viewModel != MongoDataViewModel.List && (
            <Stack direction="row">
              <SliderSize
                min={1}
                max={5}
                notice="mongodb editor grid column count"
                step={1}
                handleValue={(v) => {
                  setGridColCount(v);
                }}
                storeKey="mongo-table-list-grid-col-count"
                name="column"
              />
              <SliderSize
                name="height"
                min={200}
                max={1000}
                notice="mongodb editor height"
                step={10}
                handleValue={(v) => {
                  setEditorHeight(v);
                }}
                storeKey="mongo-table-editor-height"
              />
            </Stack>
          )
        }
      />
      {viewModel == MongoDataViewModel.List && (
        <MongoTableList
          setFilter={setFilter}
          filter={filter}
          sort={sort}
          reloadOP={reloadOPTableList}
          loadOne={loadOne}
          modifyOP={modifyOP}
          setSort={(o) => {
            setSort(o);
          }}
          deleteOP={deleteOP}
          triggerReload={triggerReload}
          loading={loading}
          setLoading={setLoading}
          tableName={table}
        />
      )}

      {viewModel != MongoDataViewModel.List && (
        <MongoTableObject
          filter={filter}
          sort={sort}
          reloadOPDocs={reloadOPDocs}
          modifyOP={modifyOP}
          deleteOP={deleteOP}
          triggerReload={triggerReload}
          setLoading={setLoading}
          gridColCount={gridColCount}
          editorHeight={editorHeight}
          viewModel={viewModel}
        />
      )}
    </Stack>
  );
}

function MongoTableAdd(props: { addOP: (doc: BSON.Document) => Promise<void> }) {
  const { addOP } = props;
  const { showNotification } = useContext(NotificationContext);
  const handleAdd = async (doc) => {
    try {
      await addOP(doc);
    } catch (e) {
      showNotification("error", "Add failure", e);
      return;
    }
    showNotification("success", "Add OK");
  };

  return (
    <Stack
      style={{
        height: "100%",
      }}
    >
      <MongoValueEditor doc={{}} setName="add" setOP={handleAdd} needConfirm={false} />
    </Stack>
  );
}

export { MongoValueEditor, MongoTable, MongoTableAdd };
