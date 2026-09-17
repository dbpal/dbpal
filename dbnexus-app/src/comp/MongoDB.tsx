import {
  Alert,
  Box,
  Button,
  ButtonGroup,
  Chip,
  Dialog,
  DialogTitle,
  Divider,
  IconButton,
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
import { UploadIcon, ListIcon } from "./icons";
import { editor, languages, MarkerSeverity } from "monaco-editor";
import { DeleteForeverIcon, FilterAltIcon, FaArrowsAltV, FaUpload } from "./icons";
import {
  bsonValue2MongoBson,
  formatMongoBsonStr,
  MongoBsonBinaryAllSubType,
  mongoBsonCompilerOwner,
  MongoBsonError,
  parseMongoBson,
} from "./MongoBsonCompiler";
import {
  Dispatch,
  ReactNode,
  SetStateAction,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { BSON, EJSON } from "bson";

import { ContentCopyIcon } from "./icons";
import {
  modalStyle,
  reloadOK,
  keyNotInTable,
  EscDownEvent,
  modalHandleClose,
  FieldAndGridField,
} from "./Common";
import {
  ArrowUpwardIcon,
  ArrowDownwardIcon,
  BsFillLightbulbFill,
  DataObjectIcon,
} from "./icons";
import {
  AppColumnMenu,
  ExportTableOnTheFilterAndSort,
  ModalHeader,
  ResetColumnWidth,
  SelectPageSize,
} from "./CommonComp";
import {
  DocsAndCount,
  handleCopyToClipboard,
  dataGridStyle,
  pageSizeOptions,
  shrinkAble,
  dataGridheaderFontSize,
  secondConfirm,
  MonacoEditor,
  extractFieldAndIDFromMouseEvent,
  DataSourceKind,
  AppColumnMenuCtx,
  columnHeaderStyle,
  gridColumnHeaderSelectClassName,
  allGridHeaderTitle,
  clickDataGridColumn,
  TabContent,
  DbnameContext,
  newEmptyResp,
  arrayToHex,
} from "./helper";
import {
  AppDataGridPagination,
  AppDataGridToolBar,
  ExportQueryResultToFile,
  ListenElementSize,
  RefreshButton,
  SimpleFlexEditor,
  SliderSize,
  StackRowCenter,
} from "./CommonComp2";
import { message } from "@tauri-apps/plugin-dialog";
import {
  MongoDataViewModel,
  useColumnWidthStore,
  useContextMenu,
  useNotification,
  usePageSize,
  UseSizeConfig,
  useSizeStore,
  useTableColumnVisibleStore,
  useTabSize,
  useMongoViewModelStore,
  useMongoEditorViewModelStore,
  useMongoTableFilterOp,
  useTabsManage,
} from "./store";
import { commands, MuiDataGridData } from "./bindings";
import { AddTableModal } from "./LeftTable";
import {
  DataGrid,
  GridCallbackDetails,
  GridPaginationModel,
  GridSlotsComponent,
  GridSlotsComponentsProps,
  useGridApiRef,
} from "./datagridExport";
import { useDebounce } from "use-debounce";
import { isEqual } from "lodash-es";
import { MongoTableImportFile } from "./CreateTable";

const mongoID = "_id";

const mongoBsonKeywords =
  /\b(as|int|long|date|timestamp|decimal|binary|generic|function|byte_array|dbName|md5|encrypted|column|sensitive|int8_array|float32_array|user_define|true|false|null|undefined|code)\b/i;
function mongoBsonAutoCompletion(): languages.CompletionList {
  const generateSubTypes = MongoBsonBinaryAllSubType.allValues().map(
    (e): languages.CompletionItem => {
      return {
        label: e,
        insertText: e,
        kind: languages.CompletionItemKind.Operator,
        range: undefined,
      };
    },
  );

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

const mongoBsonLanguage = "mongoBson";

function initMongoBsonLanguage() {
  monaco.languages.register({ id: mongoBsonLanguage });
  monaco.languages.setMonarchTokensProvider(mongoBsonLanguage, {
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

  monaco.languages.registerDocumentFormattingEditProvider(mongoBsonLanguage, {
    provideDocumentFormattingEdits(model, options) {
      const text = model.getValue();
      try {
        const result = formatMongoBsonStr(text, options.tabSize);
        return [{ range: model.getFullModelRange(), text: result }];
      } catch (e) {
        if (e instanceof MongoBsonError) {
          message(e.msgWithLoc, { kind: "error" });
        } else {
          message(String(e), { kind: "error" });
        }
      }
    },
  });

  monaco.languages.registerCompletionItemProvider(mongoBsonLanguage, {
    provideCompletionItems: () => {
      return mongoBsonAutoCompletion();
    },
  });
}

initMongoBsonLanguage();

function MongoBsonEditor(props: {
  doc: BSON.Document;
  setOP?: (doc: any) => Promise<void>;
  delOP?: () => Promise<void>;
}) {
  let { doc, setOP, delOP } = props;
  const editor = useRef<editor.IStandaloneCodeEditor>(null);
  const [loading, setLoading] = useState(false);
  const tabSize = useTabSize();
  const { showNotification } = useNotification();
  const [editorValue, setEditorValue] = useState(
    (function () {
      try {
        const c = bsonValue2MongoBson(doc).format(tabSize);
        return c;
      } catch (e) {
        showNotification({
          severity: "error",
          title: "format mongo value failed",
          message: e,
        });
      }
    })(),
  );
  const handleEditorDidMount = useCallback((ce: editor.IStandaloneCodeEditor) => {
    editor.current = ce;
    ce.onDidChangeModelContent(() => {
      setEditorValue(ce.getValue());
    });
  }, []);

  const [editorValueDebounced] = useDebounce(editorValue, 1000);

  return (
    <Stack style={{ flex: 1 }}>
      <div
        style={{
          flex: 1,
        }}>
        <MonacoEditor
          language={mongoBsonLanguage}
          value={editorValue}
          onMount={handleEditorDidMount}
          barLeft={
            <StackRowCenter>
              {setOP && (
                <IconButton
                  loading={loading}
                  disabled={(function () {
                    let x = undefined;
                    try {
                      x = parseMongoBson(editorValueDebounced).compile();
                      monaco.editor.setModelMarkers(
                        editor.current.getModel(),
                        mongoBsonCompilerOwner,
                        [],
                      );
                    } catch (e) {
                      if (e instanceof MongoBsonError) {
                        monaco.editor.setModelMarkers(
                          editor.current.getModel(),
                          mongoBsonCompilerOwner,
                          [
                            {
                              severity: MarkerSeverity.Error,
                              message: e.msg,
                              startLineNumber: e?.loc.start.line,
                              startColumn: e?.loc.start.column + 1,
                              endLineNumber: e?.loc.end.line,
                              endColumn: e?.loc.end.column + 1,
                            },
                          ],
                        );
                      } else {
                        console.error("compile should not throw other error", e);
                      }

                      return true;
                    }

                    return isEqual(x, doc);
                  })()}
                  onClick={() => {
                    setLoading(true);
                    let doc = undefined;
                    try {
                      doc = parseMongoBson(editor.current.getValue()).compile();
                    } catch (e) {
                      showNotification({
                        severity: "error",
                        title: "compile value failed",
                        message: e,
                      });
                      return;
                    }
                    setOP(doc).finally(() => {
                      setLoading(false);
                    });
                  }}>
                  <FaUpload />
                </IconButton>
              )}
              {delOP && (
                <IconButton
                  loading={loading}
                  onClick={() => {
                    setLoading(true);
                    delOP().finally(() => {
                      setLoading(false);
                    });
                  }}
                  color="error">
                  <DeleteForeverIcon />
                </IconButton>
              )}
            </StackRowCenter>
          }
        />
      </div>
    </Stack>
  );
}

function MongoValueEditor(props: {
  doc: BSON.Document;
  setOP?: (doc: BSON.Document) => Promise<void>;
  delOP?: () => Promise<void>;
  needConfirm: boolean;
  viewModel?: MongoDataViewModel;
}) {
  const { doc, delOP, setOP, needConfirm, viewModel: viewModelPassed } = props;
  const tabSize = useTabSize();
  const { viewModel, setViewModel } = useMongoEditorViewModelStore();
  const confirmAndSet = setOP
    ? async (doc: BSON.Document) => {
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

  const getViewModel = useCallback(() => {
    if (!viewModelPassed || viewModelPassed == MongoDataViewModel.TableList) {
      return viewModel;
    } else {
      return viewModelPassed;
    }
  }, [viewModel, viewModelPassed]);

  return (
    <Stack style={{ flex: 1 }}>
      {!viewModelPassed && (
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
          value={EJSON.stringify(doc, null, tabSize)}
          modifyOP={confirmAndSet}
          delOP={confirmAndDel}
        />
      )}
      {getViewModel() == MongoDataViewModel.Object && (
        <MongoBsonEditor doc={doc} setOP={confirmAndSet} delOP={confirmAndDel} />
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
  someNotice?: ReactNode;
  loading: boolean;
  table: string;
  isTable: boolean;
}) {
  const {
    handleReload,
    handleFilter,
    handleSort,
    sort,
    filter,
    someNotice,
    loading,
    table,
    isTable,
  } = props;
  const { dbName } = useContext(DbnameContext);
  const [showModifyFilter, setShowModifyFilter] = useState(false);
  const [showModifySort, setShowModifySort] = useState(false);
  const [modalBox, setModalBox] = useState(null);
  const tabSize = useTabSize();
  const { viewModel, setViewModel } = useMongoViewModelStore();
  const { appendTab } = useTabsManage();
  return (
    <Stack
      direction="row"
      style={{
        justifyContent: "space-between",
        alignItems: "center",
      }}>
      <Stack direction="row" style={{ flexWrap: "wrap", alignItems: "center" }}>
        <RefreshButton loading={loading} color="primary" onClick={handleReload} />
        <Tooltip title={bsonValue2MongoBson(filter).format(tabSize)}>
          <Chip
            label={
              viewModel == MongoDataViewModel.Object
                ? bsonValue2MongoBson(filter).format(tabSize)
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
        <Tooltip title={bsonValue2MongoBson(sort).format(tabSize)}>
          <Chip
            label={
              viewModel == MongoDataViewModel.Object
                ? bsonValue2MongoBson(sort).format(tabSize)
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

        {isTable && <AddTableModal kind={DataSourceKind.Mongo} tableName={table} />}
        {isTable && (
          <IconButton
            color="success"
            onClick={() => {
              appendTab(
                new TabContent(
                  table,
                  <MongoTableImportFile hasTableName={table} />,
                  <UploadIcon />,
                  dbName,
                  DataSourceKind.Mongo,
                ),
              );
            }}>
            <UploadIcon />
          </IconButton>
        )}
        <ExportTableOnTheFilterAndSort
          isTable={isTable}
          para={(function () {
            const whereStr = arrayToHex(BSON.serialize(filter));
            const sortStr = arrayToHex(BSON.serialize(sort));
            return { filter: whereStr, sort: sortStr };
          })()}
          tableName={table}
          extraUI=<>
            <Typography>
              export table to file base on current setting,filter and sort
            </Typography>
            <Alert severity="info"> page and page_size will not be ignored </Alert>
          </>
        />

        {someNotice}
      </Stack>

      {viewModel != MongoDataViewModel.TableList && (
        <Stack direction="row" style={{ alignItems: "center" }}>
          <SliderSize config={UseSizeConfig.gridCol} />
          <SliderSize config={UseSizeConfig.editorHeight} />
        </Stack>
      )}
      <StackRowCenter>
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
          <Button
            onClick={() => {
              setViewModel(MongoDataViewModel.TableList);
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
              viewModel={viewModel}
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
              viewModel={viewModel}
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
  field: string;
  setSort: Dispatch<SetStateAction<object>>;
  setShowFilter;
}) {
  const { delOP, content, field, setShowFilter, setSort } = props;
  const { showNotification } = useNotification();
  return (
    <MenuList>
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
          setShowFilter();
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
  );
}

function MongoTableListMenuFilter(props: {
  handleClose;
  loadValue: () => Promise<any>;
  setFilter;
  field;
}) {
  const { handleClose, loadValue, setFilter, field } = props;
  const compares = ["$eq", "$ne", "$gt", "$gte", "$lt", "$lte"];
  const { filter, setFilter: setCompare } = useMongoTableFilterOp();
  const { showNotification } = useNotification();
  const [value, setValue] = useState(null);
  const [doc, setDoc] = useState({});
  const tabSize = useTabSize();
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
    const compareObj = {};
    if (filter == "starts") {
      compareObj[field] = { $regex: `^${value}`, $options: "i" };
      setDoc(compareObj);
      return;
    }
    if (filter == "ends") {
      compareObj[field] = { $regex: `${value}$`, $options: "i" };
      setDoc(compareObj);
      return;
    }
    if (filter == "contains") {
      compareObj[field] = { $regex: `${value}`, $options: "i" };
      setDoc(compareObj);
      return;
    }
    if (filter == "null") {
      compareObj[field] = null;
      setDoc(compareObj);
      return;
    }
    if (filter == "not null") {
      compareObj[field] = { $ne: null };
      setDoc(compareObj);
      return;
    }
    compareObj[filter] = value;
    const doc = {};
    doc[field] = compareObj;
    setDoc(doc);
  }, [filter, field, value]);

  const getColor = (condition) => {
    return filter === condition ? "success" : "default";
  };
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
            color={getColor(x)}
          />
        ))}
      </Stack>
      <StackRowCenter>
        {["null", "not null"].map((condition) => (
          <Chip
            key={condition}
            label={condition}
            color={getColor(condition)}
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
              color={getColor(e)}
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
        <Typography color="success" style={{ flexShrink: 1 }}>
          {bsonValue2MongoBson(doc).format(tabSize)}
        </Typography>
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
  value: string;
  modifyOP?: (x: object) => Promise<void>;
  delOP?: () => Promise<void>;
}) {
  const { value, modifyOP, delOP } = props;
  const [newValue, setNewValue] = useState(value);
  const [loading, setLoading] = useState(false);
  const [newValueD] = useDebounce(newValue, 2000);

  let oldObject = useMemo(() => {
    return EJSON.parse(value);
  }, [value]);

  return (
    <SimpleFlexEditor
      readOnly={!modifyOP}
      value={newValue}
      language="json"
      handleValue={(v) => {
        setNewValue(v);
      }}
      barLeft=<Stack direction="row" style={{ alignItems: "center" }}>
        {modifyOP && (
          <IconButton
            onClick={() => {
              setLoading(true);
              modifyOP(EJSON.parse(newValue)).finally(() => {
                setLoading(false);
              });
            }}
            disabled={(function (): boolean {
              let x: object = null;
              try {
                x = EJSON.parse(newValueD);
              } catch {
                return true;
              }
              return isEqual(x, oldObject);
            })()}
            loading={loading}>
            <FaUpload />
          </IconButton>
        )}
        {delOP && (
          <IconButton
            onClick={async () => {
              setLoading(true);
              delOP().finally(() => {
                setLoading(false);
              });
            }}
            loading={loading}
            color="error">
            <DeleteForeverIcon />
          </IconButton>
        )}
      </Stack>
    />
  );
}

interface BsonJsonPaginationProps {
  total: number;
  setPage: (page: number) => void;
  length: number;
  page: number;
  gridColCount: number;
  viewModel: MongoDataViewModel;
}

function BsonJsonPagination({
  total,
  setPage,
  length,
  page,
  gridColCount,
  viewModel,
}: BsonJsonPaginationProps) {
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
        language={mongoBsonLanguage}
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
    uuid_field: '0fe7263c138e480ea9e1d3869857dbca' as binary dbName,
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
  setSort: Dispatch<SetStateAction<object>>;
  setFilter: Dispatch<SetStateAction<object>>;
  loadOne: (id: any) => Promise<BSON.Document>;
  modifyOP?: (doc: BSON.Document) => Promise<void>;
  deleteOP?: (ids: any[] | any) => Promise<number>;
  triggerReload;
  loading: boolean;
  setLoading: Dispatch<SetStateAction<boolean>>;
  tableName: string;
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
  } = props;
  const { dbName } = useContext(DbnameContext);
  const [page, setPage] = useState(0);
  const { mongo: pageSize, setPageSize } = usePageSize();
  const { showNotification } = useNotification();
  const [resp, setResp] = useState<MuiDataGridData>(newEmptyResp());
  const [selectModifyDoc, setSelectModifyDoc] = useState<BSON.Document | null>(null);
  const { getColumnWidth } = useColumnWidthStore();
  const [showFilter, setShowFilter] = useState<{ field: string; id: any }>(null);
  const [selectedColumns, setSelectColumns] = useState<Set<string>>(new Set());
  const apiRef = useGridApiRef();

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
                  onClick={(event) => {
                    event.stopPropagation();
                    clickDataGridColumn(apiRef, event, field.field, setSelectColumns);
                  }}
                  style={{
                    alignItems: "center",
                    cursor: "default",
                    ...columnHeaderStyle,
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
    [apiRef, getColumnWidth, page, pageSize, reloadOP, setLoading, showNotification],
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
                setShowFilter={() => {
                  setShowFilter({ field, id: EJSON.parse(row[mongoID]) });
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
  }, [handleDel, resp, setSort, setUpMenu]);
  const slots = useMemo(() => {
    return {
      columnMenu: AppColumnMenu,
      toolbar: () => {
        return <AppDataGridToolBar titleName={tableName} />;
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
              <ExportQueryResultToFile />
              <DoubleClickToModify />
            </Stack>
            <AppDataGridPagination showPageSelect />
          </Stack>
        );
      },
    } as Partial<GridSlotsComponent>;
  }, [tableName]);

  const [modalBox, setModalBox] = useState(null);
  const { getColumnVisibility, setColumnVisibility } = useTableColumnVisibleStore();
  const onColumnVisibilityModelChange = useCallback(
    (e) => {
      setColumnVisibility(dbName, tableName, e);
    },
    [dbName, setColumnVisibility, tableName],
  );
  const getCellClassName = useCallback(
    ({ field }) => {
      if (selectedColumns.has(field)) {
        return gridColumnHeaderSelectClassName;
      }
    },
    [selectedColumns],
  );
  const onCellClick = useCallback(() => {
    setSelectColumns(new Set());
    allGridHeaderTitle(apiRef, (x) => {
      x.classList.remove(gridColumnHeaderSelectClassName);
    });
  }, [apiRef]);
  const columnVisibilityModel = useMemo(
    () => getColumnVisibility(dbName, tableName),
    [dbName, getColumnVisibility, tableName],
  );
  const columnMenuValue = useMemo(
    () => ({
      selectColumns: resp.fields
        .filter((f) => selectedColumns.has(f.field))
        .map((f) => f.field),
    }),
    [resp, selectedColumns],
  );

  return (
    <Stack style={{ height: "100%" }}>
      <AppColumnMenuCtx value={columnMenuValue}>
        <DataGrid
          columnVisibilityModel={columnVisibilityModel}
          onColumnVisibilityModelChange={onColumnVisibilityModelChange}
          getCellClassName={getCellClassName}
          onCellClick={onCellClick}
          apiRef={apiRef}
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
        />
      </AppColumnMenuCtx>

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
      {showFilter && (
        <Dialog
          open={true}
          onClose={() => {
            setShowFilter(null);
          }}
          fullWidth
          maxWidth="sm">
          <DialogTitle> quick filer </DialogTitle>
          <MongoTableListMenuFilter
            loadValue={async () => {
              return loadOne(showFilter.id).then((ret) => ret[showFilter.field]);
            }}
            setFilter={setFilter}
            field={showFilter.field}
            handleClose={() => {
              setShowFilter(null);
            }}
          />
        </Dialog>
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
  setLoading: Dispatch<SetStateAction<boolean>>;
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
                flexDirection: "column",
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
    dbName: string,
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
    return await commands.mongoShowCollectionTableList(dbName, table, para);
  }

  static reloadDocs(dbName: string, table: string, filter, sort, page, pageSize) {
    let para: MongoShowTablePara = MongoTableHandler.makeShowTablePara(
      filter,
      sort,
      page,
      pageSize,
    );
    return commands.mongoShowCollectionDocs(dbName, table, para).then((x) => {
      const docs = x.docs.map((e) => BSON.deserialize(new Uint8Array(e)));
      const count = x.count;
      return new DocsAndCount(docs, count);
    });
  }

  static loadOne(dbName: string, table: string) {
    return async (id): Promise<BSON.Document> => {
      return commands
        .mongoGetOneDocumentById(dbName, table, JSON.stringify(EJSON.serialize(id)))
        .then((x) => {
          return BSON.deserialize(new Uint8Array(x));
        });
    };
  }

  static modifyOP(dbName: string, table: string) {
    return async (doc) => {
      return commands.mongoUpdateOne(dbName, table, BSON.serialize(doc));
    };
  }
  static deleteOP(dbName, table) {
    return async (ids) => {
      if (!Array.isArray(ids)) {
        ids = [ids];
      }
      return commands.mongoDeleteIds(
        dbName,
        table,
        JSON.stringify(EJSON.serialize(ids)),
      );
    };
  }
}
function MongoTable(props: { table: string; isTable: boolean }) {
  const { dbName } = useContext(DbnameContext);
  const { table, isTable } = props;
  const modifyOP = isTable ? MongoTableHandler.modifyOP(dbName, table) : undefined;
  const loadOne = MongoTableHandler.loadOne(dbName, table);
  const deleteOP = isTable ? MongoTableHandler.deleteOP(dbName, table) : undefined;
  const [filter, setFilter] = useState<BSON.Document>({});
  const [sort, setSort] = useState<BSON.Document>({});
  const { viewModel } = useMongoViewModelStore();

  const [triggerReload, setTriggerReload] = useState(0);
  const [loading, setLoading] = useState(false);

  const { getSize } = useSizeStore();

  const reloadTable = useCallback(
    (page: number, pageSize: number) => {
      return MongoTableHandler.reloadTableList(
        dbName,
        table,
        filter,
        sort,
        page,
        pageSize,
      );
    },
    [filter, sort, table, dbName],
  );
  useEffect(() => {}, [reloadTable]);
  const reloadDocs = useCallback(
    (page: number, pageSize: number) => {
      return MongoTableHandler.reloadDocs(dbName, table, filter, sort, page, pageSize);
    },
    [filter, sort, table, dbName],
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
        loading={loading}
        table={table}
        isTable={isTable}
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
      <MongoValueEditor doc={{}} setOP={handleAdd} needConfirm={false} />
    </Stack>
  );
}

export { MongoValueEditor, MongoTable, MongoTableAdd };
