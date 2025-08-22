import Editor from '@monaco-editor/react';
import {
  Box,
  Button,
  ButtonGroup,
  Chip,
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
} from '@mui/material';
import ListIcon from '@mui/icons-material/List';
import { loader } from '@monaco-editor/react';
import { CancellationToken, editor, languages, Position } from 'monaco-editor';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import FilterAltIcon from '@mui/icons-material/FilterAlt';
import { FaArrowsAltV } from 'react-icons/fa';
import {
  bsonValue2MongoBson,
  formatMongoBsonStr,
  MongoBsonBinaryAllSubType,
  MongoBsonError,
  parseMongoBson,
} from './MongoBsonCompiler';
import { v4 as uuidv4 } from 'uuid';
import {
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';

import { BSON, EJSON } from 'bson';
import types from './ApiStruct';
import { NotificationContext } from './Notification';
import { TbJson } from 'react-icons/tb';
import {
  DataGrid,
  GridPaginationModel,
  GridSlotsComponentsProps,
} from '@mui/x-data-grid';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import {
  modalStyle,
  reloadOK,
  keyNotInTable,
  EscDownEvent,
  EditorBar,
  SimpleFlexSizeEditor,
} from './Common';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';
import { BsFillLightbulbFill } from 'react-icons/bs';
import DataObjectIcon from '@mui/icons-material/DataObject';
import {
  MenuResetColumnWidths,
  ModalHeader,
  pageSizeOptions,
  ResizeWrapper,
  SelectPageSize,
} from './CommonComp';
import {
  calMenuStyle,
  columnWidths,
  ContextMenuContext,
  DocsAndCount,
  filterStyle,
  gridStyle,
  handleCopyToClipboard,
  JsonError,
  resetMuiDataRespWidth,
  saveAndSetSize,
  SettingContext,
  validJson,
  VisiableContext,
} from './helper';

enum MongoDataViewModel {
  List,
  Object,
  Json,
}
const mongoBsonKeywords =
  /\b(as|int|long|date|timestamp|decimal|binary|generic|function|byte_array|uuid|md5|encrypted|column|sensitive|int8_array|float32_array|user_define|true|false|null|undefined|code)\b/;
function mongoBsonAutoCompletion(): languages.CompletionList {
  let generateSubTypes = MongoBsonBinaryAllSubType.allValues().map((e) => {
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
        label: 'as',
        kind: languages.CompletionItemKind.Keyword,
        insertText: 'as',
        range: undefined,
      },
      {
        label: 'int',
        kind: languages.CompletionItemKind.Class,
        insertText: 'int',
        range: undefined,
      },
      {
        label: 'long',
        kind: languages.CompletionItemKind.Class,
        insertText: 'long',
        range: undefined,
      },
      {
        label: 'date',
        kind: languages.CompletionItemKind.Class,
        insertText: 'date',
        range: undefined,
      },
      {
        label: 'timestamp',
        kind: languages.CompletionItemKind.Class,
        insertText: 'timestamp',
        range: undefined,
      },
      {
        label: 'decimal',
        kind: languages.CompletionItemKind.Class,
        insertText: 'decimal',
        range: undefined,
      },
      {
        label: 'binary',
        kind: languages.CompletionItemKind.Class,
        insertText: 'binary',
        range: undefined,
      },
      {
        label: 'ObjectID',
        kind: languages.CompletionItemKind.Class,
        insertText: 'ObjectID',
        range: undefined,
      },
      {
        label: 'code',
        kind: languages.CompletionItemKind.Class,
        insertText: 'code',
        range: undefined,
      },
      {
        label: 'true',
        kind: languages.CompletionItemKind.Constant,
        insertText: 'true',
        range: undefined,
      },
      {
        label: 'false',
        kind: languages.CompletionItemKind.Constant,
        insertText: 'false',
        range: undefined,
      },

      {
        label: 'float32_array',
        kind: languages.CompletionItemKind.Class,
        insertText: 'float32',
        range: undefined,
      },
      {
        label: 'int8_array',
        kind: languages.CompletionItemKind.Class,
        insertText: 'int8',
        range: undefined,
      },
    ],
  };
}
const languageXXX = 'mongoBson-7135637247';

function initMongoBsonLanguage() {
  loader
    .init()
    .then((monaco) => {
      monaco.languages.register({ id: languageXXX });
      monaco.languages.setMonarchTokensProvider(languageXXX, {
        ignoreCase: true,
        tokenizer: {
          root: [
            [mongoBsonKeywords, 'keyword'],
            [/\"(?:\\.|[^\\\"])*\"/, 'string'],
            [/\'(?:\\.|[^\\\'])*\'/, 'string'],
            [/\`(?:\\.|[^\\\`])*\`/, 'string'],
          ],
        },
        brackets: [
          { open: '{', close: '}', token: 'delimiter.curly' },
          { open: '[', close: ']', token: 'delimiter.parenthesis' },
        ],
      });

      monaco.languages.registerDocumentFormattingEditProvider(languageXXX, {
        provideDocumentFormattingEdits(model, options, _token) {
          let text = model.getValue();
          try {
            const result = formatMongoBsonStr(text, options.tabSize);
            return [{ range: model.getFullModelRange(), text: result }];
          } catch (e) {
            if (e instanceof MongoBsonError) {
              alert(e.msgWithLoc);
            } else {
              alert(e.toString());
            }
          }
        },
      });

      monaco.languages.registerCompletionItemProvider(languageXXX, {
        provideCompletionItems: (
          _model: editor.ITextModel,
          _position: Position,
          _context: any,
          _token: CancellationToken
        ) => {
          return mongoBsonAutoCompletion();
        },
      });
    })
    .catch((e) => {
      console.log('init monaco language error:', e);
    });
}

initMongoBsonLanguage();

function MongoBsonEditor(props: {
  doc: BSON.Document;
  setOP?: (doc) => Promise<void>;
  setOPName?;
  delOP?;
}) {
  let { doc, setOP, delOP, setOPName } = props;
  if (!setOPName) {
    setOPName = 'set';
  }
  let editor = useRef<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position>(null);
  const [err, setErr] = useState<MongoBsonError>(null);
  const setting = useContext(SettingContext);
  const [loading, setLoading] = useState(false);
  const newDoc = useRef(doc);
  const handleEditorDidMount = useCallback(
    (ce: editor.IStandaloneCodeEditor, _monaco) => {
      editor.current = ce;
      ce.onDidChangeModelContent((_event) => {
        let text = ce.getValue();
        try {
          const doc = parseMongoBson(text).compile();
          setErr(null);
          newDoc.current = doc;
        } catch (e) {
          setErr(e);
        }
      });
      ce.onDidChangeCursorPosition((event) => {
        setPos(event.position);
      });
    },
    []
  );

  const { showNotification } = useContext(NotificationContext);
  const [content, setContent] = useState('{\n}');
  useEffect(() => {
    try {
      let c = bsonValue2MongoBson(doc).format();
      setContent(c);
    } catch (e) {
      showNotification('error', 'format value failed', e);
    }
  }, []);
  const options = useMemo((): editor.IStandaloneEditorConstructionOptions => {
    return {
      readOnly: !setOP,
      fontSize: setting.editorFontSize,
      ...setting.editorOption,
    };
  }, [setting]);
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
        ></Editor>
      </div>
      <EditorBar
        pos={pos}
        left={
          <Stack direction="row" style={{ alignItems: 'center' }} spacing={1}>
            {setOP && (
              <Button
                variant="contained"
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
                variant="contained"
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
                style={{ cursor: 'pointer' }}
                onClick={() => {
                  if (err.loc) {
                    editor.current.setPosition({
                      lineNumber: err.loc.start.line,
                      column: err.loc.end.column,
                    });
                    editor.current.focus();
                    editor.current.revealLines(
                      err.loc.start.line,
                      err.loc.end.line
                    );
                  }
                }}
              >
                {err.msgWithLoc}
              </Typography>
            ) : (
              <></>
            )}
          </Stack>
        }
      ></EditorBar>
    </Stack>
  );
}

let oldMongoValueEdiorMode = MongoDataViewModel.Object;

function MongoValueEditor(props: {
  doc: BSON.Document;
  setOP?: (doc: BSON.Document) => Promise<void>;
  setName?;
}) {
  let { doc, setOP, setName } = props;
  if (!setName) {
    setName = 'set';
  }
  const [viewModel, setViewModel] = useState(oldMongoValueEdiorMode);
  useEffect(() => {
    oldMongoValueEdiorMode = viewModel;
  }, [viewModel]);
  return (
    <Stack flex={1}>
      <Stack direction="row">
        <ButtonGroup sx={{ gap: 0.5 }}>
          <IconButton
            disabled={viewModel == MongoDataViewModel.Object}
            onClick={() => {
              setViewModel(MongoDataViewModel.Object);
            }}
            edge="start"
          >
            <DataObjectIcon style={{ fontSize: '1.7rem' }} />
          </IconButton>
          <IconButton
            onClick={() => {
              setViewModel(MongoDataViewModel.Json);
            }}
            disabled={viewModel == MongoDataViewModel.Json}
            edge="start"
          >
            <TbJson style={{ fontSize: '2.2rem' }} />
          </IconButton>
        </ButtonGroup>
      </Stack>
      {viewModel == MongoDataViewModel.Json && (
        <JsonEditor
          value={JSON.parse(EJSON.stringify(doc))}
          modifyOP={setOP}
          modifyOPName={setName}
        ></JsonEditor>
      )}
      {viewModel == MongoDataViewModel.Object && (
        <MongoBsonEditor
          doc={doc}
          setOP={setOP}
          setOPName={setName}
        ></MongoBsonEditor>
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
  addOP: (doc: BSON.Document) => Promise<void>;
  someNotice?: ReactNode;
  nonse;
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
  } = props;

  const [showModifyFilter, setShowModifyFilter] = useState(false);
  const [showModifySort, setShowModifySort] = useState(false);
  const [loading, setLoading] = useState(false);
  const doReload = () => {
    setLoading(true);
    handleReload().finally(() => {
      setLoading(false);
    });
  };
  useEffect(() => {
    handleViewModel(viewModel);
  }, [viewModel]);
  const [modalBox, setModalBox] = useState(null);
  useEffect(() => {
    if (modalBox) {
      let c = saveAndSetSize(modalBox, 'mongo-filter-sort');
      return c;
    }
  }, [modalBox]);
  return (
    <Stack
      direction="row"
      style={{
        justifyContent: 'space-between',
        alignItems: 'center',
      }}
    >
      <Stack
        direction="row"
        spacing={1}
        style={{ flexWrap: 'wrap', alignItems: 'center' }}
      >
        <Button
          loading={loading}
          variant="contained"
          color="primary"
          onClick={doReload}
        >
          reload
        </Button>

        <Chip
          label={
            oldMongoValueEdiorMode == MongoDataViewModel.Object
              ? bsonValue2MongoBson(filter).format()
              : EJSON.stringify(filter)
          }
          onClick={() => {
            setShowModifyFilter(true);
          }}
          color="primary"
          icon={<FilterAltIcon />}
          style={{ ...filterStyle.style }}
        ></Chip>

        <Chip
          label={
            oldMongoValueEdiorMode == MongoDataViewModel.Object
              ? bsonValue2MongoBson(sort).format()
              : EJSON.stringify(sort)
          }
          onClick={() => {
            setShowModifySort(true);
          }}
          color="secondary"
          icon={<FaArrowsAltV />}
          style={{ ...filterStyle.style }}
        ></Chip>
        {someNotice}
      </Stack>
      <Stack direction="row">
        <ButtonGroup sx={{ gap: 0.5 }}>
          <IconButton
            disabled={viewModel == MongoDataViewModel.Object}
            onClick={() => {
              handleViewModel(MongoDataViewModel.Object);
            }}
            edge="start"
          >
            <DataObjectIcon style={{ fontSize: '1.7rem' }} />
          </IconButton>
          <IconButton
            onClick={() => {
              handleViewModel(MongoDataViewModel.List);
            }}
            disabled={viewModel == MongoDataViewModel.List}
            edge="start"
          >
            <ListIcon style={{ fontSize: '2rem' }} />
          </IconButton>
          <IconButton
            onClick={() => {
              handleViewModel(MongoDataViewModel.Json);
            }}
            disabled={viewModel == MongoDataViewModel.Json}
            edge="start"
          >
            <TbJson style={{ fontSize: '2.2rem' }} />
          </IconButton>
        </ButtonGroup>
      </Stack>

      {showModifyFilter && (
        <Modal
          open={showModifyFilter}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setShowModifyFilter(false);
          }}
        >
          <Box
            sx={{ resize: 'both', ...modalStyle }}
            width="65%"
            ref={setModalBox}
            height="45vh"
            className="modal-animation"
          >
            <ModalHeader title="Modify Filter"></ModalHeader>
            <MongoValueEditor
              setOP={async (doc) => {
                handleFilter(doc);
                setShowModifyFilter(false);
              }}
              doc={filter}
            ></MongoValueEditor>
          </Box>
        </Modal>
      )}

      {showModifySort && (
        <Modal
          open={showModifySort}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setShowModifySort(false);
          }}
        >
          <Box
            sx={{ resize: 'both', ...modalStyle }}
            width="65%"
            ref={setModalBox}
            height="45vh"
            className="modal-animation"
          >
            <ModalHeader title="Modify Sort"></ModalHeader>
            <MongoValueEditor
              setOP={async (doc) => {
                handleSort(doc);
                setShowModifySort(false);
              }}
              doc={sort}
            ></MongoValueEditor>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function renderId(_id): any {
  return EJSON.serialize(_id);
}

function MongoTableListContextMenu(props: {
  delOP?: () => void;
  content;
  field;
  setFilter;
  loadDoc: () => Promise<BSON.Document>;
  setSort;
  resetColunmWidthCallBack;
}) {
  const {
    delOP,
    content,
    field,
    setFilter,
    loadDoc,
    setSort,
    resetColunmWidthCallBack,
  } = props;
  const [showFilter, setShowFilter] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [showViewJson, setShowViewJson] = useState(false);
  const [modalBox, setModalBox] = useState(null);

  useEffect(() => {
    if (modalBox) {
      let c = saveAndSetSize(modalBox, 'mongo--listview-json');
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
              <ContentCopyIcon></ContentCopyIcon>
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
              <ContentCopyIcon></ContentCopyIcon>
            </ListItemIcon>
            <ListItemText> Copy </ListItemText>
          </MenuItem>
        )}

        <MenuItem
          onClick={() => {
            handleCopyToClipboard(field, showNotification);
          }}
        >
          <ListItemIcon>
            <ContentCopyIcon></ContentCopyIcon>
          </ListItemIcon>
          <ListItemText> Copy Field </ListItemText>
        </MenuItem>
        <Divider></Divider>
        <MenuItem
          onClick={() => {
            let doc = {};
            doc[field] = 1;
            setSort(doc);
          }}
        >
          <ListItemIcon>
            <ArrowUpwardIcon></ArrowUpwardIcon>
          </ListItemIcon>
          <ListItemText> {field} ASC </ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            let doc = {};
            doc[field] = -1;
            setSort(doc);
          }}
        >
          <ListItemIcon>
            <ArrowDownwardIcon></ArrowDownwardIcon>
          </ListItemIcon>
          <ListItemText> {field} DESC </ListItemText>
        </MenuItem>
        <Divider></Divider>
        <MenuItem
          onClick={() => {
            setShowFilter(true);
          }}
        >
          <ListItemIcon>
            <FilterAltIcon></FilterAltIcon>
          </ListItemIcon>
          <ListItemText> Filter </ListItemText>
        </MenuItem>
        {delOP && (
          <MenuItem
            onClick={() => {
              delOP();
            }}
          >
            <ListItemIcon>
              <DeleteForeverIcon></DeleteForeverIcon>
            </ListItemIcon>
            <ListItemText> Delete </ListItemText>
          </MenuItem>
        )}
        <MenuResetColumnWidths
          reset={resetColunmWidthCallBack}
        ></MenuResetColumnWidths>
      </MenuList>

      {showViewJson && (
        <Modal
          open={showViewJson}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setShowViewJson(false);
          }}
        >
          <Box
            sx={{ resize: 'both', ...modalStyle }}
            ref={setModalBox}
            width="75%"
            className="modal-animation"
          >
            <ModalHeader title="View Json"></ModalHeader>
            <SimpleFlexSizeEditor
              value={content}
              language="json"
            ></SimpleFlexSizeEditor>
          </Box>
        </Modal>
      )}

      {showFilter && (
        <Modal
          open={showFilter}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setShowFilter(false);
          }}
        >
          <Box sx={modalStyle} width="75%" className="modal-animation">
            <ModalHeader title="Quick Filter"></ModalHeader>
            <MongoTableListMenuFilter
              loadValue={async () => {
                return loadDoc().then((x) => x[field]);
              }}
              setFilter={setFilter}
              field={field}
              handleClose={() => {
                setShowFilter(false);
              }}
            ></MongoTableListMenuFilter>
          </Box>
        </Modal>
      )}
    </div>
  );
}

function MongoTableListMenuFilter(props: {
  handleClose;
  loadValue: () => Promise<any>;
  setFilter;
  field;
}) {
  const { handleClose, loadValue, setFilter, field } = props;
  const compares = ['$eq', '$ne', '$gt', '$gte', '$lt', '$lte'];
  const [compare, setCompare] = useState(compares[0]);
  const { showNotification } = useContext(NotificationContext);
  const [value, setValue] = useState(null);
  const [doc, setDoc] = useState({});
  useEffect(() => {
    loadValue()
      .then((v) => {
        setValue(v);
      })
      .catch((e) => {
        showNotification('error', 'get field value failed', e);
      });
  }, []);

  useEffect(() => {
    let c = {};
    c[compare] = value;
    let doc = {};
    doc[field] = c;
    setDoc(doc);
  }, [compare, value]);

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1}>
        {compares.map((x) => (
          <Chip
            key={x}
            onClick={() => {
              setCompare(x);
            }}
            label={x}
            size="small"
            color={compare === x ? 'primary' : 'default'}
          ></Chip>
        ))}
      </Stack>
      {bsonValue2MongoBson(doc).format()}
      <Stack direction="row">
        <Button
          variant="contained"
          onClick={() => {
            setFilter(doc);
            handleClose();
          }}
        >
          set
        </Button>
      </Stack>
    </Stack>
  );
}

function MongoTableJson(props: {
  filter: BSON.Document;
  sort: BSON.Document;
  realodOPDocs: (
    filter: BSON.Document,
    sort: BSON.Document,
    page: number,
    pageSize: number
  ) => Promise<DocsAndCount>;
  modifyOP?: (doc: BSON.Document) => Promise<void>;
  deleteOP?: (ids: any[] | any) => Promise<void>;
  nonse;
}) {
  const { filter, sort, realodOPDocs, modifyOP, deleteOP, nonse } = props;
  const [jsons, setJsons] = useState<DocsAndCount>({ docs: [], count: 0 });
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(10);
  const { showNotification } = useContext(NotificationContext);
  const handleReload = async (showOK) => {
    realodOPDocs(filter, sort, page, pageSize)
      .then((resp) => {
        setJsons(resp);
        if (showOK) {
          reloadOK(showNotification);
        }
      })
      .catch((e) => {
        showNotification('error', 'fetch data failed', e);
      });
  };
  const handleDetele = async (ids: any[]) => {
    try {
      await deleteOP(ids);
    } catch (e) {
      showNotification('success', 'Delete failure', e);
      return;
    }
    handleReload(false);
    showNotification('success', 'Delete OK');
  };

  useEffect(() => {
    handleReload(false);
  }, [page, pageSize, filter, sort]);
  useEffect(() => {
    handleReload(true);
  }, [nonse]);
  return (
    <Stack spacing={1}>
      {jsons.docs.map((doc) => {
        return (
          <ResizeWrapper
            sizeKeyKind="height"
            sizeKey="mongo-view-data-json"
            height="300px"
            key={renderId(doc['_id'])}
          >
            <JsonEditor
              value={doc}
              modifyOP={
                modifyOP
                  ? async (doc) => {
                      try {
                        await modifyOP(doc);
                      } catch (e) {
                        showNotification('error', 'modify failed', e);
                        return;
                      }
                      handleReload(false);
                      showNotification('success', 'Modify OK');
                    }
                  : undefined
              }
              handleDetele={
                deleteOP
                  ? async () => {
                      await handleDetele(doc['_id']);
                    }
                  : undefined
              }
            ></JsonEditor>
          </ResizeWrapper>
        );
      })}

      {/* // here is pagination */}
      <BsonJsonPagination
        pageSize={pageSize}
        total={jsons.count}
        length={jsons.docs.length}
        page={page}
        setPage={setPage}
        setPageSize={setPageSize}
      ></BsonJsonPagination>
    </Stack>
  );
}

function DoubleClickToModify() {
  return (
    <Tooltip title="double click a row to modify." arrow>
      <BsFillLightbulbFill />
    </Tooltip>
  );
}
function JsonEditor(props: {
  value: object;
  modifyOP?: (x: object) => Promise<void>;
  modifyOPName?: string;
  handleDetele?: () => Promise<void>;
}) {
  const { value, modifyOP, handleDetele, modifyOPName } = props;
  const [newValue, setNewValue] = useState(EJSON.stringify(value, null, 2));
  const [jsonErr, setJsonErr] = useState<JsonError>(null);
  const [loading, setLoading] = useState(false);
  const [jsonErrPos, setJsonErrPos] = useState<Position>(null);
  useEffect(() => {
    (async () => {
      try {
        await validJson(newValue);
      } catch (e) {
        setJsonErr(e);
        return;
      }
      try {
        EJSON.parse(newValue);
      } catch (e) {
        setJsonErr(e.toString());
        return;
      }
      setJsonErr(null);
    })();
  }, [newValue]);

  return (
    <SimpleFlexSizeEditor
      readOnly={!modifyOP}
      value={newValue}
      language="json"
      errPos={jsonErrPos}
      handleValue={(v) => {
        setNewValue(v);
      }}
      barLeft=<Stack direction="row" alignItems="center" spacing={1}>
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
            variant="contained"
          >
            {modifyOPName ? modifyOPName : 'set'}
          </Button>
        )}
        {handleDetele && (
          <Button
            onClick={() => {
              setLoading(true);
              handleDetele().finally(() => {
                setLoading(false);
              });
            }}
            loading={loading}
            variant="contained"
            color="error"
          >
            delete
          </Button>
        )}

        {jsonErr && (
          <Typography
            color="error"
            style={{ cursor: 'pointer' }}
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
    ></SimpleFlexSizeEditor>
  );
}

function BsonJsonPagination({
  pageSize,
  total,
  setPage,
  length,
  page,
  setPageSize,
}) {
  return (
    <Stack direction="row" spacing={1} alignItems="center">
      <Pagination
        page={page}
        defaultPage={1}
        count={Math.ceil(total / pageSize)}
        showFirstButton
        showLastButton
        onChange={(_e, page) => {
          setPage(page);
        }}
      />
      <Typography>
        result {length} of {total}
      </Typography>
      <SelectPageSize
        options={[10, 20]}
        pageSize={pageSize}
        handleChange={(size) => {
          setPageSize(size);
        }}
      ></SelectPageSize>
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
    pageSize: number
  ) => Promise<types.MuiDataGridData>;
  setSort;
  setFilter;
  loadOne: (id: any) => Promise<BSON.Document>;
  modifyOP?: (doc: BSON.Document) => Promise<void>;
  deleteOP?: (ids: any[] | any) => Promise<void>;
  nonse;
}) {
  const {
    filter,
    sort,
    reloadOP,
    setSort,
    loadOne,
    modifyOP,
    deleteOP,
    nonse,
    setFilter,
  } = props;
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(useContext(SettingContext).pageSize);
  const { showNotification } = useContext(NotificationContext);
  const [resp, setResp] = useState<types.MuiDataGridData | null>(null);
  const [selectModifyDoc, setSelectModifyDoc] = useState<BSON.Document | null>(
    null
  );
  const [loading, setLoading] = useState(false);

  const handleReload = async (showOK) => {
    setLoading(true);
    reloadOP(filter, sort, page + 1, pageSize)
      .then((r) => {
        let has_null = false;
        r.data.forEach((e) => {
          e['_id'] = EJSON.parse(e['_id']);
          e[keyNotInTable] = uuidv4();
          if (!e['_id']) {
            showNotification(
              'error',
              'use null as _id which is not support right now'
            );
            has_null = true;
          }
        });
        if (has_null) {
          return;
        }
        r.fields.forEach((f) => {
          let width = columnWidths.getWidth(f.field);
          if (width) {
            f['width'] = width;
          }
        });
        setResp(r);
        if (showOK) {
          reloadOK(showNotification);
        }
      })
      .catch((e) => {
        showNotification('error', 'fetch data failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  useEffect(() => {
    handleReload(false);
  }, [page, pageSize, filter, sort]);

  useEffect(() => {
    handleReload(true);
  }, [nonse]);

  const handleLoadOne = async (id: any) => {
    try {
      let x = await loadOne(id);
      return x;
    } catch (e) {
      showNotification('error', 'fetch data failed', e);
      throw e;
    }
  };

  const getRowId = useCallback((e) => {
    return renderId(e[keyNotInTable]);
  }, []);
  const onPaginationModelChange = useCallback((m: GridPaginationModel) => {
    setPage(m.page);
    setPageSize(m.pageSize);
  }, []);
  const onRowDoubleClick = useCallback((e) => {
    handleLoadOne(e.row._id).then((doc) => {
      setSelectModifyDoc(doc);
    });
  }, []);

  const paginationModel = useMemo(() => {
    return { page, pageSize };
  }, [page, pageSize]);

  const onSortModelChange = useCallback((e) => {
    if (e.length == 0) {
      setSort({});
      return;
    }
    let s = e[0];
    if (!s.sort) {
      setSort({});
      return;
    }
    let order = {};
    order[s.field] = s.sort == 'asc' ? 1 : -1;
    setSort(order);
  }, []);

  const handleDel = deleteOP
    ? async (id: string) => {
        let del = resp.data.find((e) => e[keyNotInTable] == id);
        try {
          await deleteOP([del['_id']]);
          showNotification('success', 'delete OK');
          handleReload(false);
        } catch (e) {
          showNotification('error', 'delete failed', e);
        }
      }
    : undefined;
  const vis = useContext(VisiableContext);
  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          // let content = (target as HTMLDivElement).innerHTML;
          let field = (target as HTMLDivElement).getAttribute('data-field');
          let id = (currentTarget as HTMLDivElement).getAttribute('data-id');
          if (!field) {
            return;
          }
          let row = resp.data.find((e) => e[keyNotInTable] == id);
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
                  let doc = resp.data.find((x) => x[keyNotInTable] == id);
                  return await loadOne(doc['_id']);
                }}
                setSort={setSort}
                resetColunmWidthCallBack={() => {
                  setResp((old) => {
                    return resetMuiDataRespWidth(old);
                  });
                }}
              ></MongoTableListContextMenu>
            ),
            style: calMenuStyle(e, 200),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);

  const onColumnResize = useCallback(({ colDef, width }) => {
    columnWidths.saveColumnWidths(colDef.field, width);
  }, []);
  const setting = useContext(SettingContext);

  const [modalBox, setModalBox] = useState(null);
  useEffect(() => {
    if (modalBox) {
      let c = saveAndSetSize(modalBox, 'mongo-list-modify');
      return c;
    }
  }, [modalBox]);
  return (
    <>
      {resp && (
        <Stack spacing={1}>
          {vis && (
            <div
              style={{
                display: 'grid',
              }}
            >
              <DataGrid
                style={gridStyle}
                loading={loading}
                showCellVerticalBorder
                showColumnVerticalBorder
                slotProps={slotProps}
                getRowId={getRowId}
                rowHeight={setting.datagridRowHeight}
                columnHeaderHeight={setting.datagridHeaderHeight}
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
              ></DataGrid>
            </div>
          )}
        </Stack>
      )}
      {selectModifyDoc && (
        <Modal
          open={!!selectModifyDoc}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setSelectModifyDoc(null);
          }}
          className="modal-animation"
        >
          <Box
            sx={{ resize: 'both', ...modalStyle }}
            width="75%"
            height="60vh"
            ref={setModalBox}
            className="modal-animation"
          >
            <ModalHeader title="Modify data" />
            <MongoValueEditor
              setOP={
                modifyOP
                  ? async (doc) => {
                      try {
                        await modifyOP(doc);
                      } catch (e) {
                        showNotification('error', 'Modify failed', e);
                        throw e;
                      }
                      showNotification('success', 'Modify OK');
                      setSelectModifyDoc(null);
                      handleReload(false);
                    }
                  : undefined
              }
              doc={selectModifyDoc}
            ></MongoValueEditor>
          </Box>
        </Modal>
      )}
    </>
  );
}

function MongoTableObject(props: {
  filter: BSON.Document;
  sort: BSON.Document;
  realodOPDocs: (
    filter: BSON.Document,
    sort: BSON.Document,
    page: number,
    pageSize: number
  ) => Promise<DocsAndCount>;
  modifyOP?: (doc: BSON.Document) => Promise<void>;
  deleteOP?: (ids: any[] | any) => Promise<void>;
  nonse;
}) {
  const { filter, sort, realodOPDocs, modifyOP, deleteOP, nonse } = props;
  const [docs, setDocs] = useState<DocsAndCount>({ docs: [], count: 0 });
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(10);
  const { showNotification } = useContext(NotificationContext);
  const handleReload = async (showOK) => {
    realodOPDocs(filter, sort, page, pageSize)
      .then((resp) => {
        setDocs(resp);
        if (showOK) {
          reloadOK(showNotification);
        }
      })
      .catch((e) => {
        showNotification('error', 'fetch data failed', e);
      });
  };
  const handleDetele = async (ids: any[]) => {
    try {
      await deleteOP(ids);
    } catch (e) {
      showNotification('error', 'Delete failure', e);
    }
    showNotification('success', 'Delete OK');
    handleReload(false);
  };
  useEffect(() => {
    handleReload(false);
  }, [page, pageSize, filter, sort]);
  useEffect(() => {
    handleReload(true);
  }, [nonse]);
  return (
    <Stack spacing={1}>
      {docs.docs.map((doc) => {
        return (
          <ResizeWrapper
            key={renderId(doc._id)}
            sizeKeyKind="height"
            sizeKey="mongo-view-data"
            height="300px"
          >
            <MongoBsonEditor
              doc={doc}
              setOP={
                modifyOP
                  ? async (doc) => {
                      try {
                        await modifyOP(doc);
                      } catch (e) {
                        showNotification('error', 'Modify failed', e);
                        return;
                      }
                      showNotification('success', 'modify ok');
                      handleReload(false);
                    }
                  : undefined
              }
              delOP={
                deleteOP
                  ? async () => {
                      return await handleDetele(doc['_id']);
                    }
                  : undefined
              }
            ></MongoBsonEditor>
          </ResizeWrapper>
        );
      })}

      {/* // here is pagination */}
      <BsonJsonPagination
        total={docs.count}
        length={docs.docs.length}
        page={page}
        setPage={setPage}
        pageSize={pageSize}
        setPageSize={setPageSize}
      ></BsonJsonPagination>
    </Stack>
  );
}

let cacheViewModel = MongoDataViewModel.Object;
function MongoTable(props: {
  reloadOPTableList: (
    filter: BSON.Document,
    sort: BSON.Document,
    page: number,
    pageSize: number
  ) => Promise<types.MuiDataGridData>;
  reloadOPDocs: (
    filter: BSON.Document,
    sort: BSON.Document,
    page: number,
    pageSize: number
  ) => Promise<DocsAndCount>;
  loadOne: (id: any) => Promise<BSON.Document>;
  modifyOP?: (doc: BSON.Document) => Promise<void>;
  deleteOP?: (ids: any[] | any) => Promise<void>;
}) {
  const { reloadOPTableList, loadOne, modifyOP, reloadOPDocs, deleteOP } =
    props;
  const [filter, setFilter] = useState<BSON.Document>({});
  const [sort, setSort] = useState<BSON.Document>({});
  const [viewModel, setViewModel] = useState(cacheViewModel);
  const [nonse, setNonse] = useState(0);
  useEffect(() => {
    cacheViewModel = viewModel;
  }, [viewModel]);
  return (
    <Stack spacing={1}>
      <MongoTableToolBar
        addOP={async () => {}}
        sort={sort}
        filter={filter}
        handleFilter={(e) => {
          setFilter(e);
        }}
        handleSort={(e) => {
          setSort(e);
        }}
        handleReload={async () => {
          setNonse((prev) => prev + 1);
        }}
        handleViewModel={(e) => {
          setViewModel(e);
        }}
        viewModel={viewModel}
        nonse={nonse}
        someNotice={
          viewModel == MongoDataViewModel.List ? (
            <DoubleClickToModify></DoubleClickToModify>
          ) : null
        }
      ></MongoTableToolBar>

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
          nonse={nonse}
        ></MongoTableList>
      )}

      {viewModel == MongoDataViewModel.Object && (
        <MongoTableObject
          filter={filter}
          sort={sort}
          realodOPDocs={reloadOPDocs}
          modifyOP={modifyOP}
          deleteOP={deleteOP}
          nonse={nonse}
        ></MongoTableObject>
      )}
      {viewModel == MongoDataViewModel.Json && (
        <MongoTableJson
          filter={filter}
          sort={sort}
          realodOPDocs={reloadOPDocs}
          modifyOP={modifyOP}
          deleteOP={deleteOP}
          nonse={nonse}
        ></MongoTableJson>
      )}
    </Stack>
  );
}

function MongoTableAdd(props: {
  addOP: (doc: BSON.Document) => Promise<void>;
}) {
  let { addOP } = props;
  const { showNotification } = useContext(NotificationContext);
  const handleAdd = async (doc) => {
    try {
      await addOP(doc);
    } catch (e) {
      showNotification('error', 'Add failure', e);
      return;
    }
    showNotification('success', 'Add OK');
  };
  const [dom, setDom] = useState(null);
  useEffect(() => {
    if (dom) {
      let c = saveAndSetSize(dom, 'mongo-table-add', 'height');
      return c;
    }
  }, [dom]);
  return (
    <Stack
      style={{
        resize: 'vertical',
        overflow: 'auto',
        minHeight: '20vh',
        height: '300px',
      }}
      ref={setDom}
    >
      <MongoValueEditor
        doc={{}}
        setName="add"
        setOP={handleAdd}
      ></MongoValueEditor>
    </Stack>
  );
}

export { MongoValueEditor, MongoTable, MongoTableAdd };
