import Editor from '@monaco-editor/react';
import {
  Alert,
  Box,
  Button,
  ButtonGroup,
  CircularProgress,
  IconButton,
  Modal,
  Pagination,
  Stack,
  Typography,
} from '@mui/material';
import ListIcon from '@mui/icons-material/List';
import { loader } from '@monaco-editor/react';
import { CancellationToken, editor, languages, Position } from 'monaco-editor';
import AddIcon from '@mui/icons-material/Add';
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
import { DataGrid, GridPaginationModel } from '@mui/x-data-grid';

import {
  DocsAndCount,
  modalStyle,
  MongoDataViewModel,
  pageSizeOptions,
  SelectPageSize,
  editorInitHeight,
  progressBarSize,
  SettingsContext,
  VisiableContext,
  reloadOK,
  keyNotInTable,
} from './Common';

import DataObjectIcon from '@mui/icons-material/DataObject';
import { datagridHeaderHeight, datagridRowHeight } from './CommonComp';

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
  let [editor, setEditor] = useState<editor.IStandaloneCodeEditor>();
  const [pos, setPos] = useState<Position>(null);
  const [docHasChange, setDocHasChange] = useState(false);
  const [err, setErr] = useState<MongoBsonError>(null);
  const { theme } = useContext(SettingsContext);

  const handleEditorDidMount = useCallback(
    (ce: editor.IStandaloneCodeEditor, monaco) => {
      setEditor(ce);
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
  const [content, _setcontent] = useState(bsonValue2MongoBson(doc).format());
  const { showNotification } = useContext(NotificationContext);
  const [initHeigth, _] = useState(
    editorInitHeight(
      content,
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
          value={content}
        ></Editor>
      </div>

      <Stack direction="row" justifyContent="space-between">
        <Stack direction="row" alignContent="flex-start" spacing={1}>
          {buttons.map((button) => {
            return (
              //@ts-ignore
              <Button
                size="small"
                key={button.buttonConent}
                variant={button.variant}
                color={button.color}
                onClick={() => {
                  try {
                    let doc = parseMongoBson(editor.getValue()).compile();
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

        <Button
          size="small"
          variant="contained"
          color="primary"
          onClick={doReload}
        >
          Reload
        </Button>
        <Button
          size="small"
          variant="contained"
          color="primary"
          onClick={doReload}
        >
          <AddIcon></AddIcon>
        </Button>
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
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const [filter, setFilter] = useState<BSON.Document>({});
  const [orderBy, setOrderby] = useState<BSON.Document>({});
  // we have two page becuase one start from `0` and one from `1`
  const [pageTableList, setPageTableList] = useState(0);
  const [pageObject, setPageObject] = useState(1);

  const [selectModifyDoc, setSelectModifyDoc] = useState<BSON.Document | null>(
    null
  );
  const [docs, setDocs] = useState<DocsAndCount | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [viewModel, setViewModel] = useState(MongoDataViewModel.Object);
  const [pageSize, setPageSize] = useState(0);

  useEffect(() => {
    setPageSize(
      viewModel == MongoDataViewModel.Object
        ? 10
        : useContext(SettingsContext).pageSize
    );
  }, []);

  const handleReload = (showOK: boolean) => {
    const handleReloadTableList = async () => {
      reloadOPTableList(filter, orderBy, pageTableList + 1, pageSize)
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

    const handleReloadDocs = async () => {
      realodOPDocs(filter, orderBy, pageObject, pageSize)
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
    if (viewModel === MongoDataViewModel.List) {
      handleReloadTableList();
    } else {
      handleReloadDocs();
    }
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

  const handleLoadOne = async (id: any) => {
    try {
      let x = await loadOne(id);
      return x;
    } catch (e) {
      showNotification('error', 'fetch data failed', e);
      throw e;
    }
  };
  useEffect(() => {
    handleReload(true);
  }, [pageTableList, pageObject, pageSize, viewModel, filter, orderBy]);

  const getRowId = useCallback((e) => {
    return renderId(e[keyNotInTable]);
  }, []);
  const onPaginationModelChange = useCallback((m: GridPaginationModel) => {
    setPageTableList(m.page);
    setPageSize(m.pageSize);
  }, []);
  const onRowDoubleClick = useCallback((e) => {
    handleLoadOne(e.row._id).then((doc) => {
      setSelectModifyDoc(doc);
    });
  }, []);
  const paginationModel = useMemo(() => {
    return { page: pageTableList, pageSize: pageSize };
  }, [pageTableList, pageSize]);
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

  const vis = useContext(VisiableContext);
  return (
    <Stack
      spacing={1}
      style={{
        width: '99%',
      }}
    >
      <MongoTableListToolBar
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
          handleReload(true);
        }}
        handleViewModel={(e) => {
          setViewModel(e);
        }}
        viewModel={viewModel}
      ></MongoTableListToolBar>

      {/* // datagrid modeal */}
      {resp && viewModel == MongoDataViewModel.List && (
        <Stack spacing={1}>
          {vis && (
            <div style={{ display: 'grid' }}>
              <DataGrid
                getRowId={getRowId}
                rowHeight={datagridRowHeight}
                columnHeaderHeight={datagridHeaderHeight}
                checkboxSelection
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
              ></DataGrid>
            </div>
          )}
        </Stack>
      )}
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
      {/* // docs modal  */}
      {docs && viewModel == MongoDataViewModel.Object && (
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
              page={pageObject}
              defaultPage={1}
              count={Math.ceil(docs.total / pageSize)}
              showFirstButton
              showLastButton
              onChange={(_e, page) => {
                setPageObject(page);
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
