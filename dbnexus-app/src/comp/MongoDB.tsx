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
  CSSProperties,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import {
  Binary,
  BSON,
  BSONRegExp,
  Code,
  Decimal128,
  Double,
  EJSON,
  Int32,
  Long,
  ObjectId,
  Timestamp,
} from 'bson';
import types from './ApiStruct';
import { NotificationContext } from './Notification';
import {
  DataGrid,
  GridPaginationModel,
  GridSlotsComponentsProps,
} from '@mui/x-data-grid';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import {
  DocsAndCount,
  modalStyle,
  SettingContext,
  reloadOK,
  keyNotInTable,
  EscDownEvent,
} from './Common';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';

import DataObjectIcon from '@mui/icons-material/DataObject';
import {
  MenuResetColumnWidths,
  ModalHeader,
  pageSizeOptions,
  SelectPageSize,
} from './CommonComp';
import {
  calMenuStyle,
  columnWidths,
  ContextMenuContext,
  filterStyle,
  handleCopyToClipboard,
  resetMuiDataRespWidth,
  saveAndSetSize,
  VisiableContext,
} from './helper';

enum MongoDataViewModel {
  List,
  Object,
}
const mongoBsonKeywords =
  /\b(as|int|long|date|timestamp|decimal|binary|generic|function|byte_array|uuid|md5|encrypted|column|sensitive|vector|user_define|true|false|null|undefined|code|float32|int8)\b/;
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
        label: 'float32',
        kind: languages.CompletionItemKind.Class,
        insertText: 'float32',
        range: undefined,
      },
      {
        label: 'int8',
        kind: languages.CompletionItemKind.Class,
        insertText: 'int8',
        range: undefined,
      },
    ],
  };
}

function initMongoBsonLanguage() {
  const language = 'mongoBson';
  loader
    .init()
    .then((monaco) => {
      monaco.languages.register({ id: language });
      monaco.languages.setMonarchTokensProvider(language, {
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

      monaco.languages.registerDocumentFormattingEditProvider(language, {
        provideDocumentFormattingEdits(model, options, _token) {
          let text = model.getValue();
          try {
            const result = formatMongoBsonStr(text, options.tabSize);
            return [{ range: model.getFullModelRange(), text: result }];
          } catch (e) {
            console.log(e);
            if (e instanceof MongoBsonError) {
              alert(e.msgWithLoc);
            } else {
              alert(e.toString());
            }
          }
        },
      });

      monaco.languages.registerCompletionItemProvider(language, {
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

interface MongoBsonEditorCallback {
  color?: string;
  disabled?: (docHasChange: boolean) => boolean;
  variant?: string;
  callBack: (doc: BSON.Document) => void;
  buttonText: string;
}

function MongoBsonEditor(props: {
  doc: BSON.Document;
  buttons: MongoBsonEditorCallback[];
  heightName: string;
}) {
  let { doc, buttons, heightName } = props;
  let editor = useRef<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position>(null);
  const [docHasChange, setDocHasChange] = useState(false);
  const [err, setErr] = useState<MongoBsonError>(null);
  const setting = useContext(SettingContext);
  const [loading, setLoading] = useState(false);

  const handleEditorDidMount = useCallback(
    (ce: editor.IStandaloneCodeEditor, _monaco) => {
      editor.current = ce;
      ce.onDidChangeModelContent((_event) => {
        let text = ce.getValue();
        setDocHasChange(true);
        try {
          parseMongoBson(text).compile();
          setErr(null);
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
  const content = useRef(bsonValue2MongoBson(doc).format());
  const { showNotification } = useContext(NotificationContext);
  const options = useMemo((): editor.IStandaloneEditorConstructionOptions => {
    return {
      fontSize: setting.editorFontSize,
      ...setting.editorOption,
    };
  }, [setting]);
  const divDom = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const x1 = saveAndSetSize(divDom.current, `mongobsonEditor-${heightName}`);
    return () => {
      x1();
    };
  }, []);

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        resize: 'vertical',
        overflow: 'auto',
        minHeight: '150px',
      }}
      ref={divDom}
    >
      <div
        style={{
          flexGrow: 1,
          flexShrink: 1,
          minHeight: 0,
        }}
      >
        <Editor
          width="99%"
          height="99%"
          onMount={handleEditorDidMount}
          language="mongoBson"
          value={content.current}
          options={options}
          theme={setting.editorTheme()}
        ></Editor>
      </div>

      <Stack direction="row" justifyContent="space-between">
        <Stack direction="row" style={{ alignItems: 'center' }} spacing={1}>
          {buttons.map((button) => {
            return (
              //@ts-ignore
              <Button
                loading={loading}
                key={button.buttonText}
                variant={button.variant}
                color={button.color}
                onClick={() => {
                  setLoading(true);
                  try {
                    let doc = parseMongoBson(
                      editor.current.getValue()
                    ).compile();
                    button.callBack(doc);
                  } catch (e) {
                    setLoading(false);
                    showNotification('error', 'mongo bson compile failed', e);
                    console.log('mongo bson compile failed', e);
                    return;
                  }
                  setLoading(false);
                }}
                disabled={
                  button.disabled ? button.disabled(docHasChange) : false
                }
              >
                {button.buttonText}
              </Button>
            );
          })}

          {err ? (
            <Typography color="error">{err.msgWithLoc}</Typography>
          ) : (
            <></>
          )}
        </Stack>
        <Stack width="40%" spacing={1} direction="row" alignContent="center">
          {pos && (
            <>
              <span> Ln {pos.lineNumber} </span>
              <span> Col {pos.column} </span>
            </>
          )}
        </Stack>
      </Stack>
    </div>
  );
}

function MongoDBModify(props: {
  doc: BSON.Document;
  setOP?: (doc: BSON.Document) => Promise<void>;
  buttonName: string;
  handleClose?;
}) {
  let { doc, setOP, buttonName, handleClose } = props;

  const { showNotification } = useContext(NotificationContext);

  const handleModify = (doc) => {
    setOP(doc)
      .then(() => {
        showNotification('success', 'Modify OK');

        if (handleClose) {
          handleClose();
        }
      })
      .catch((e) => {
        showNotification('error', 'Modify failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      <MongoBsonEditor
        doc={doc}
        heightName="modify"
        buttons={
          setOP
            ? [
                {
                  color: 'primary',
                  callBack: (doc) => {
                    handleModify(doc);
                  },
                  variant: 'contained',
                  buttonText: buttonName,
                  disabled: (changed) => {
                    return !changed;
                  },
                },
              ]
            : []
        }
      ></MongoBsonEditor>
    </Stack>
  );
}

function MongoTableListToolBar(props: {
  handleReload: () => Promise<void>;
  handleFilter: (doc: BSON.Document) => void;
  handleSort: (doc: BSON.Document) => void;
  sort: BSON.Document;
  filter: BSON.Document;
  viewModel: MongoDataViewModel;
  handleViewModel: (doc: MongoDataViewModel) => void;
  addOP: (doc: BSON.Document) => Promise<void>;

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

  return (
    <Stack
      direction="row"
      style={{
        justifyContent: 'space-between',
        alignItems: 'center',
      }}
    >
      <Stack direction="row" spacing={1} style={{ flexWrap: 'wrap' }}>
        <Button
          loading={loading}
          variant="contained"
          color="primary"
          onClick={doReload}
        >
          reload
        </Button>

        <Chip
          label={bsonValue2MongoBson(filter).format()}
          onClick={() => {
            setShowModifyFilter(true);
          }}
          color="primary"
          icon={<FilterAltIcon />}
          style={{ ...filterStyle.style }}
        ></Chip>

        <Chip
          label={bsonValue2MongoBson(sort).format()}
          onClick={() => {
            setShowModifySort(true);
          }}
          color="secondary"
          icon={<FaArrowsAltV />}
          style={{ ...filterStyle.style }}
        ></Chip>
      </Stack>
      <Stack direction="row">
        <ButtonGroup>
          <IconButton
            disabled={viewModel == MongoDataViewModel.Object}
            onClick={() => {
              handleViewModel(MongoDataViewModel.Object);
            }}
            edge="start"
          >
            <DataObjectIcon />
          </IconButton>
          <IconButton
            onClick={() => {
              handleViewModel(MongoDataViewModel.List);
            }}
            disabled={viewModel == MongoDataViewModel.List}
            edge="start"
          >
            <ListIcon />
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
          <Box sx={modalStyle} width="65%">
            <ModalHeader title="Modify Filter"></ModalHeader>
            <MongoDBModify
              buttonName="set"
              setOP={async (doc) => {
                handleFilter(doc);
              }}
              handleClose={() => {
                setShowModifyFilter(false);
              }}
              doc={filter}
            ></MongoDBModify>
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
          <Box sx={modalStyle} width="65%">
            <ModalHeader title="Modify Sort"></ModalHeader>
            <MongoDBModify
              buttonName="set"
              setOP={async (doc) => {
                handleSort(doc);
              }}
              handleClose={() => {
                setShowModifySort(false);
              }}
              doc={sort}
            ></MongoDBModify>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function renderId(_id): any {
  if (_id instanceof Int32) {
    return _id.toString();
  }
  // Timestamp 必须在Long前面 可能timestamp继承到Long
  if (_id instanceof Timestamp) {
    return _id.toString();
  }
  if (_id instanceof Long) {
    return _id.toString();
  }

  if (_id instanceof Float32Array) {
    return _id.toString();
  }
  if (_id instanceof Int8Array) {
    return _id.toString();
  }

  if (_id instanceof Code) {
    return _id.code;
  }
  if (_id instanceof Date) {
    return _id.toLocaleString();
  }
  if (_id instanceof BSONRegExp) {
    return _id.pattern + _id.options;
  }
  if (_id instanceof RegExp) {
    return _id.toString();
  }
  if (_id instanceof ObjectId) {
    return _id.toHexString();
  }
  if (_id instanceof Double) {
    return _id.value;
  }
  if (_id instanceof Decimal128) {
    return _id.toString();
  }
  if (_id instanceof Binary) {
    return _id.toString('hex');
  }
  if (_id instanceof Array) {
    return `[${_id.map((e) => renderId(e)).join(',')}]`;
  }
  if (_id instanceof Object) {
    let ret = '{';
    let first = true;
    for (const key in _id) {
      if (!first) {
        ret += ',';
      }
      ret += `${key}:${renderId(_id[key])}`;
      first = false;
    }
    ret += '}';
    return ret;
  }
  if (['boolean', 'number', 'string'].includes(typeof _id)) {
    return _id.toString();
  }
  console.log('_id type not handled', _id);
  return _id.toString();
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
  return (
    <div>
      <MenuList>
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
          <Box sx={modalStyle} width="75%">
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
  deleteOP?: (ids: any[]) => Promise<void>;
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
        if (r.fields.length > 0) {
          r.fields[0]['renderCell'] = function (value) {
            let _id = value.row._id;
            return renderId(_id);
          };
        }
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
          let row = { ...resp.data.find((e) => e[keyNotInTable] == id) };
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
  const gridStyle = useMemo(() => {
    return { height: 'auto', maxHeight: '78vh' } as CSSProperties;
  }, []);
  return (
    <>
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
        >
          <Box sx={modalStyle} width="75%">
            <MongoDBModify
              setOP={
                modifyOP
                  ? async (doc) => {
                      try {
                        await modifyOP(doc);
                      } catch (e) {
                        throw e;
                      }
                      setSelectModifyDoc(null);
                      handleReload(false);
                    }
                  : undefined
              }
              doc={selectModifyDoc}
              buttonName="modify"
            ></MongoDBModify>
          </Box>
        </Modal>
      )}

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
                disableColumnFilter
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
  deleteOP?: (ids: any[]) => Promise<void>;
  nonse;
}) {
  const { filter, sort, realodOPDocs, modifyOP, deleteOP, nonse } = props;
  const [docs, setDocs] = useState<DocsAndCount | null>(null);
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
    deleteOP(ids)
      .then(() => {
        showNotification('success', 'Delete OK');
        handleReload(false);
      })
      .catch((e) => {
        showNotification('error', 'Delete failure', e);
      });
  };

  useEffect(() => {
    handleReload(false);
  }, [page, pageSize, filter, sort]);
  useEffect(() => {
    handleReload(true);
  }, [nonse]);
  return (
    <>
      {docs && (
        <Stack spacing={1}>
          {docs.docs.map((doc) => {
            return (
              <MongoBsonEditor
                heightName="object-view"
                key={doc._id}
                doc={doc}
                buttons={
                  modifyOP && deleteOP
                    ? [
                        {
                          buttonText: 'set',
                          callBack: (doc) => {
                            modifyOP(doc)
                              .then(() => {
                                showNotification('success', 'Modify OK');
                              })
                              .catch((e) => {
                                showNotification('error', 'Modify failure', e);
                              });
                          },
                        },
                        {
                          buttonText: 'delete',
                          color: 'error',
                          callBack: (_) => {
                            handleDetele([doc._id]);
                          },
                        },
                      ]
                    : []
                }
              ></MongoBsonEditor>
            );
          })}
          {/* // here is pagination */}
          <Stack direction="row" spacing={1} alignItems="center">
            <Pagination
              page={page}
              defaultPage={1}
              count={Math.ceil(docs.total / pageSize)}
              showFirstButton
              showLastButton
              onChange={(_e, page) => {
                setPage(page);
              }}
            />
            <Typography>
              result {docs.docs.length} of {docs.total}
            </Typography>
            <SelectPageSize
              options={[10, 20]}
              pageSize={pageSize}
              handleChange={(size) => {
                setPageSize(size);
              }}
            ></SelectPageSize>
          </Stack>
        </Stack>
      )}
    </>
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
  realodOPDocs: (
    filter: BSON.Document,
    sort: BSON.Document,
    page: number,
    pageSize: number
  ) => Promise<DocsAndCount>;
  loadOne: (id: any) => Promise<BSON.Document>;
  modifyOP?: (doc: BSON.Document) => Promise<void>;
  deleteOP?: (ids: any[]) => Promise<void>;
}) {
  const { reloadOPTableList, loadOne, modifyOP, realodOPDocs, deleteOP } =
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
      <MongoTableListToolBar
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
      ></MongoTableListToolBar>

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
          realodOPDocs={realodOPDocs}
          modifyOP={modifyOP}
          deleteOP={deleteOP}
          nonse={nonse}
        ></MongoTableObject>
      )}
    </Stack>
  );
}

function MongoTableAdd(props: {
  addOP: (doc: BSON.Document) => Promise<void>;
}) {
  let { addOP } = props;
  const { showNotification } = useContext(NotificationContext);
  const handleAdd = (doc) => {
    addOP(doc)
      .then(() => {
        showNotification('success', 'Add OK');
      })
      .catch((e) => {
        showNotification('error', 'Add failure', e);
      });
  };

  return (
    <MongoBsonEditor
      doc={{}}
      heightName="add"
      buttons={[
        {
          buttonText: 'add',
          variant: 'contained',
          disabled: (changed) => {
            return !changed;
          },
          callBack: (doc) => {
            handleAdd(doc);
          },
        },
      ]}
    ></MongoBsonEditor>
  );
}

export { MongoBsonEditor, MongoTable, MongoTableAdd };
