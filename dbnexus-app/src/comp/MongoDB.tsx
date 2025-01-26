import Editor from '@monaco-editor/react';
import {
  Alert,
  Box,
  Button,
  ButtonGroup,
  CircularProgress,
  IconButton,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Pagination,
  Paper,
  Stack,
  Typography,
} from '@mui/material';
import ListIcon from '@mui/icons-material/List';
import { loader } from '@monaco-editor/react';
import { CancellationToken, editor, languages, Position } from 'monaco-editor';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';

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
  MongoDataViewModel,
  editorInitHeight,
  progressBarSize,
  SettingsContext,
  VisiableContext,
  reloadOK,
  keyNotInTable,
  ContextMenuContext,
} from './Common';

import DataObjectIcon from '@mui/icons-material/DataObject';
import {
  datagridHeaderHeight,
  datagridRowHeight,
  pageSizeOptions,
  SelectPageSize,
} from './CommonComp';
import {
  calMenuStyle,
  cloumnWidths,
  handleCopyToClipboard,
  saveColumnWidths,
} from './helper';

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
  loader.init().then((monaco) => {
    monaco.languages.register({ id: 'mongoBson' });
    monaco.languages.setMonarchTokensProvider('mongoBson', {
      ignoreCase: true,
      tokenizer: {
        root: [
          [mongoBsonKeywords, 'keyword'],
          [/\"(?:\\.|[^\\\"])*\"/, 'string'],
          [/\'(?:\\.|[^\\\'])*\'/, 'string'],
          [/\`(?:\\.|[^\\\`])*\`/, 'string'],
        ],
      },
    });

    monaco.languages.registerDocumentFormattingEditProvider('mongoBson', {
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

    monaco.languages.registerCompletionItemProvider('mongoBson', {
      provideCompletionItems: (
        _model: editor.ITextModel,
        _position: Position,
        _context: any,
        _token: CancellationToken
      ) => {
        return mongoBsonAutoCompletion();
      },
    });
  });
}

initMongoBsonLanguage();

interface MongoBsonEditorCallback {
  color?: string;
  disabled?: (docHasChange: boolean) => boolean;
  variant?: string;
  callBack: (doc: BSON.Document) => void;
  buttonConent: string;
}

function MongoBsonEditor(props: {
  doc: BSON.Document;
  buttons: MongoBsonEditorCallback[];
}) {
  let { doc, buttons } = props;
  let editor = useRef<editor.IStandaloneCodeEditor>();
  const [pos, setPos] = useState<Position>(null);
  const [docHasChange, setDocHasChange] = useState(false);
  const [err, setErr] = useState<MongoBsonError>(null);
  const { theme } = useContext(SettingsContext);

  const handleEditorDidMount = useCallback(
    (ce: editor.IStandaloneCodeEditor, monaco) => {
      editor.current = ce;
      ce.onDidChangeModelContent((_event) => {
        let text = ce.getValue();
        setDocHasChange(true);
        try {
          parseMongoBson(text);
        } catch (e) {
          setErr(e);
        }
      });
      ce.onDidChangeCursorPosition((event) => {
        setPos(event.position);
      });
      if (theme == 'dark') {
        monaco.editor.setTheme('vs-dark');
      }
    },
    []
  );
  const content = useRef(bsonValue2MongoBson(doc).format());
  const { showNotification } = useContext(NotificationContext);
  const [initHeigth, _] = useState(
    editorInitHeight(
      content.current,
      useContext(SettingsContext).editorFontSize * 1.2,
      150
    )
  );

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        resize: 'vertical',
        overflow: 'auto',
        height: initHeigth + 'px',
      }}
    >
      <div
        style={{
          height: '100px',
          flexGrow: 1,
          flexShrink: 1,
        }}
      >
        <Editor
          onMount={handleEditorDidMount}
          language="mongoBson"
          value={content.current}
        ></Editor>
      </div>

      <Stack direction="row" justifyContent="space-between">
        <Stack direction="row" alignContent="flex-start" spacing={1}>
          {buttons.map((button) => {
            return (
              //@ts-ignore
              <Button
                key={button.buttonConent}
                variant={button.variant}
                color={button.color}
                onClick={() => {
                  try {
                    let doc = parseMongoBson(
                      editor.current.getValue()
                    ).compile();
                    button.callBack(doc);
                  } catch (e) {
                    showNotification('error', 'mongo bson compile failed', e);
                    return;
                  }
                }}
                disabled={
                  button.disabled ? button.disabled(docHasChange) : false
                }
              >
                {button.buttonConent}
              </Button>
            );
          })}

          {err == null && <Alert severity="success">Valid Document</Alert>}
          {!!err && <Alert severity="error">{err.msgWithLoc}</Alert>}
        </Stack>
        <Stack
          width="40%"
          spacing={1}
          direction="row"
          alignContent="flex-start"
        >
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
  setOP: (doc: BSON.Document) => Promise<void>;
  buttonName: string;
  handleClose?;
}) {
  let { doc, setOP, buttonName, handleClose } = props;
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);

  const handleModify = (doc) => {
    setLoading(true);
    setOP(doc)
      .then(() => {
        showNotification('success', 'Modify OK');
        setLoading(false);
        if (handleClose) {
          handleClose();
        }
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'Modify failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      {loading && <CircularProgress size={progressBarSize} />}
      <MongoBsonEditor
        doc={doc}
        buttons={[
          {
            color: 'primary',
            callBack: (doc) => {
              handleModify(doc);
            },
            variant: 'contained',
            buttonConent: buttonName,
            disabled: (changed) => {
              return !changed;
            },
          },
        ]}
      ></MongoBsonEditor>
    </Stack>
  );
}

function MongoTableListToolBar(props: {
  handleReload: () => Promise<void>;
  handleFilter: (doc: BSON.Document) => void;
  handleOrderBy: (doc: BSON.Document) => void;
  orderBy: BSON.Document;
  filter: BSON.Document;
  viewModel: MongoDataViewModel;
  handleViewModel: (doc: MongoDataViewModel) => void;
  addOP: (doc: BSON.Document) => Promise<void>;
  style?: CSSProperties;
  nonse;
}) {
  const {
    handleReload,
    handleFilter,
    handleOrderBy,
    orderBy,
    filter,
    viewModel,
    handleViewModel,
    style,
  } = props;

  const [showModifyFilter, setShowModifyFilter] = useState(false);
  const [showModifyOrderBy, setShowModifyOrderBy] = useState(false);
  const [loading, setLoading] = useState(false);
  const doReload = () => {
    setLoading(true);
    handleReload().then(() => {
      setLoading(false);
    });
  };
  useEffect(() => {
    handleViewModel(viewModel);
  }, [viewModel]);

  return (
    <Stack direction="row" justifyContent="space-between" style={{ ...style }}>
      <Stack direction="row" spacing={1} alignContent="flex-end">
        <Typography
          onClick={() => {
            setShowModifyFilter(true);
          }}
          style={{ cursor: 'pointer' }}
          variant="subtitle1"
        >
          filter: {bsonValue2MongoBson(filter).format()}
        </Typography>

        {showModifyFilter && (
          <Modal
            open={showModifyFilter}
            onClose={() => {
              setShowModifyFilter(false);
            }}
            aria-labelledby="set filter"
          >
            <Box sx={modalStyle} width="800px">
              <MongoDBModify
                buttonName="Set"
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

        <Typography
          onClick={() => {
            setShowModifyOrderBy(true);
          }}
          style={{ cursor: 'pointer' }}
          variant="subtitle1"
        >
          order by: {bsonValue2MongoBson(orderBy).format()}
        </Typography>

        {showModifyOrderBy && (
          <Modal
            open={showModifyOrderBy}
            onClose={() => {
              setShowModifyOrderBy(false);
            }}
            aria-labelledby="order by"
          >
            <Box sx={modalStyle} width="800px">
              <MongoDBModify
                buttonName="Set"
                setOP={async (doc) => {
                  handleOrderBy(doc);
                }}
                handleClose={() => {
                  setShowModifyOrderBy(false);
                }}
                doc={orderBy}
              ></MongoDBModify>
            </Box>
          </Modal>
        )}

        <Button variant="contained" color="primary" onClick={doReload}>
          Reload
        </Button>
        {/* <Button
          
          variant="contained"
          color="primary"
          onClick={doReload}
        >
          <AddIcon></AddIcon>
        </Button> */}
        {loading && <CircularProgress size={progressBarSize} />}
      </Stack>
      <Stack direction="row" style={{ padding: '5px' }}>
        <ButtonGroup variant="contained" aria-label="Basic button group">
          <IconButton
            disabled={viewModel == MongoDataViewModel.Object}
            onClick={() => {
              handleViewModel(MongoDataViewModel.Object);
            }}
            edge="start"
            color="inherit"
            aria-label="object"
          >
            <DataObjectIcon />
          </IconButton>
          <IconButton
            onClick={() => {
              handleViewModel(MongoDataViewModel.List);
            }}
            disabled={viewModel == MongoDataViewModel.List}
            edge="start"
            color="inherit"
            aria-label="list"
          >
            <ListIcon />
          </IconButton>
        </ButtonGroup>
      </Stack>
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

function MongoTableListContextMenu(props: { delOP: () => void; content }) {
  const { delOP, content } = props;

  const { showNotification } = useContext(NotificationContext);
  return (
    <Paper>
      <MenuList>
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}
        >
          <ListItemIcon>
            <ContentCopyIcon></ContentCopyIcon>
          </ListItemIcon>
          <ListItemText> copy </ListItemText>
        </MenuItem>

        <MenuItem
          onClick={() => {
            delOP();
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText> delete </ListItemText>
        </MenuItem>
      </MenuList>
    </Paper>
  );
}

function MongoTableList(props: {
  filter: BSON.Document;
  orderBy: BSON.Document;
  reloadOP: (
    filter: BSON.Document,
    orderBy: BSON.Document,
    page: number,
    pageSize: number
  ) => Promise<types.MuiDataTableListResp>;
  setOrderby;
  loadOne: (id: any) => Promise<BSON.Document>;
  modifyOP: (doc: BSON.Document) => Promise<void>;
  deleteOP: (ids: any[]) => Promise<void>;
  nonse;
}) {
  const {
    filter,
    orderBy,
    reloadOP,
    setOrderby,
    loadOne,
    modifyOP,
    deleteOP,
    nonse,
  } = props;
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(
    useContext(SettingsContext).pageSize
  );
  const { showNotification } = useContext(NotificationContext);
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const [selectModifyDoc, setSelectModifyDoc] = useState<BSON.Document | null>(
    null
  );

  const handleReload = async (showOK) => {
    reloadOP(filter, orderBy, page + 1, pageSize)
      .then((r) => {
        let has_null = false;
        r.data.data.forEach((e) => {
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
        r.data.fields.forEach((f) => {
          let width = cloumnWidths[f.field];
          if (width) {
            f['width'] = width;
          }
        });
        r.data.fields[0]['renderCell'] = function (value) {
          let _id = value.row._id;
          return renderId(_id);
        };
        setResp(r);
        if (showOK) {
          reloadOK(showNotification);
        }
      })
      .catch((e) => {
        showNotification('error', 'fetch data failed', e);
      });
  };

  useEffect(() => {
    handleReload(true);
  }, [page, pageSize, filter, orderBy]);

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
      setOrderby({});
      return;
    }
    let s = e[0];
    if (!s.sort) {
      setOrderby({});
      return;
    }
    let order = {};
    order[s.field] = s.sort == 'asc' ? 1 : -1;
    setOrderby(order);
  }, []);

  const handleDel = async (id: string) => {
    let del = resp.data.data.find((e) => e[keyNotInTable] == id);
    try {
      await deleteOP([del['_id']]);
      showNotification('success', 'delete OK');
      handleReload(false);
    } catch (e) {
      showNotification('error', 'delete failed', e);
    }
  };
  const vis = useContext(VisiableContext);
  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          let content = (target as HTMLDivElement).innerHTML;
          let id = (currentTarget as HTMLDivElement).getAttribute('data-id');
          setUpMenu({
            content: (
              <MongoTableListContextMenu
                content={content}
                delOP={() => {
                  handleDel(id);
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
    saveColumnWidths(colDef.field, width);
  }, []);

  return (
    <>
      {selectModifyDoc && (
        <Modal
          open={!!selectModifyDoc}
          onClose={() => {
            setSelectModifyDoc(null);
          }}
          aria-labelledby="modify set"
        >
          <Box sx={modalStyle} width="800px">
            <MongoDBModify
              setOP={async (doc) => {
                try {
                  alert(1);
                  await modifyOP(doc);
                } catch (e) {
                  throw e;
                }
                handleReload(false);
              }}
              doc={selectModifyDoc}
              buttonName="Modify"
            ></MongoDBModify>
          </Box>
        </Modal>
      )}

      {resp && (
        <Stack spacing={1}>
          {vis && (
            <div style={{ display: 'grid' }}>
              <DataGrid
                slotProps={slotProps}
                getRowId={getRowId}
                rowHeight={datagridRowHeight}
                columnHeaderHeight={datagridHeaderHeight}
                rowCount={resp.total}
                columns={resp.data.fields}
                rows={resp.data.data}
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
  orderBy: BSON.Document;
  realodOPDocs: (
    filter: BSON.Document,
    orderBy: BSON.Document,
    page: number,
    pageSize: number
  ) => Promise<DocsAndCount>;
  modifyOP: (doc: BSON.Document) => Promise<void>;
  deleteOP: (ids: any[]) => Promise<void>;
  nonse;
}) {
  const { filter, orderBy, realodOPDocs, modifyOP, deleteOP, nonse } = props;
  const [docs, setDocs] = useState<DocsAndCount | null>(null);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(10);
  const { showNotification } = useContext(NotificationContext);
  const handleReload = async (showOK) => {
    realodOPDocs(filter, orderBy, page, pageSize)
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
  }, [page, pageSize, filter, orderBy]);
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
                key={doc._id}
                doc={doc}
                buttons={[
                  {
                    buttonConent: 'set',
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
                    buttonConent: 'delete',
                    color: 'error',
                    callBack: (_) => {
                      handleDetele([doc._id]);
                    },
                  },
                ]}
              ></MongoBsonEditor>
            );
          })}
          {/* // here is pagination */}
          <Stack direction="row" spacing={1} alignItems="flex-end">
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
              pageSize={10}
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

function MongoTable(props: {
  reloadOPTableList: (
    filter: BSON.Document,
    orderBy: BSON.Document,
    page: number,
    pageSize: number
  ) => Promise<types.MuiDataTableListResp>;
  realodOPDocs: (
    filter: BSON.Document,
    orderBy: BSON.Document,
    page: number,
    pageSize: number
  ) => Promise<DocsAndCount>;
  loadOne: (id: any) => Promise<BSON.Document>;
  modifyOP: (doc: BSON.Document) => Promise<void>;
  deleteOP: (ids: any[]) => Promise<void>;
}) {
  const { reloadOPTableList, loadOne, modifyOP, realodOPDocs, deleteOP } =
    props;
  const [filter, setFilter] = useState<BSON.Document>({});
  const [orderBy, setOrderby] = useState<BSON.Document>({});
  const [viewModel, setViewModel] = useState(MongoDataViewModel.Object);
  const [nonse, setNonse] = useState(0);
  return (
    <Stack
      spacing={1}
      style={{
        width: '99%',
      }}
    >
      <MongoTableListToolBar
        addOP={async () => {}}
        style={{ position: 'sticky', top: 0 }}
        orderBy={orderBy}
        filter={filter}
        handleFilter={(e) => {
          setFilter(e);
        }}
        handleOrderBy={(e) => {
          setOrderby(e);
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
          filter={filter}
          orderBy={orderBy}
          reloadOP={reloadOPTableList}
          loadOne={loadOne}
          modifyOP={modifyOP}
          setOrderby={(o) => {
            setOrderby(o);
          }}
          deleteOP={deleteOP}
          nonse={nonse}
        ></MongoTableList>
      )}
      {viewModel == MongoDataViewModel.Object && (
        <MongoTableObject
          filter={filter}
          orderBy={orderBy}
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
    <Stack>
      <MongoBsonEditor
        doc={{}}
        buttons={[
          {
            buttonConent: 'Add',
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
    </Stack>
  );
}

export { MongoBsonEditor, MongoTable, MongoTableAdd };
