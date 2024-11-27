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
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import {
  bsonValue2MongoBson,
  formatMongoBsonStr,
  MongoBsonBinaryAllSubType,
  MongoBsonError,
  parseMongoBson,
} from './MongoBsonCompiler';

import { useContext, useEffect, useState } from 'react';
import { BSON, ObjectId } from 'bson';
import types from './ApiStruct';
import { NotificationContext } from './Notification';
import { DataGrid } from '@mui/x-data-grid';

import {
  DocsAndCount,
  modalStyle,
  MongoDataViewModel,
  pageSizeOptions,
  SelectPageSize,
  datagridRowHeight,
  datagridHeaderHeight,
  defaultPageSize,
  editorInitHeight,
} from './common';

import DataObjectIcon from '@mui/icons-material/DataObject';
import { secondConfirm } from './Confirm';
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
        root: [[mongoBsonKeywords, 'keyword']],
      },
    });

    monaco.languages.registerDocumentFormattingEditProvider('mongoBson', {
      provideDocumentFormattingEdits(model, options, _token) {
        let text = model.getValue();
        let result = formatMongoBsonStr(text, options.tabSize);
        if (typeof result === 'string') {
          return [{ range: model.getFullModelRange(), text: result }];
        } else {
          let e = result as MongoBsonError;
          let msg = `${e.loc.start.line}:${e.loc.start.column}  ${e.msg}`;
          alert(msg);
          throw result;
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
  handleChange: (doc: BSON.Document) => void;
  buttons: MongoBsonEditorCallback[];
}) {
  let { doc, handleChange, buttons } = props;
  //@ts-ignore
  let [content, setContent] = useState(bsonValue2MongoBson(doc).format());

  const [cursorLine, setCursorLine] = useState(1);
  const [cursorColumn, setCursorColumn] = useState(1);
  const [err, setErr] = useState<MongoBsonError | null>(null);
  const [docHasChange, setDocHasChange] = useState(false);

  const handleEditorDidMount = (ce: editor.IStandaloneCodeEditor) => {
    ce.onDidChangeModelContent((_event) => {
      let text = ce.getValue();
      try {
        let d = parseMongoBson(text).compile();
        setErr(null);
        handleChange(d);
        setDocHasChange(true);
      } catch (e) {
        setErr(e);
        return;
      }
    });

    ce.onDidChangeCursorPosition((event) => {
      setCursorLine(event.position.lineNumber);
      setCursorColumn(event.position.column);
    });
  };
  const initHeigth = editorInitHeight(content);
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
          height: '20px',
          flexGrow: 1,
          flexShrink: 1,
          border: '1px gray black',
        }}
      >
        <Editor
          onMount={handleEditorDidMount}
          language='mongoBson'
          value={content}
          height='100%'
        ></Editor>
      </div>

      <Stack direction='row' justifyContent='space-between'>
        <Stack direction='row' alignContent='flex-start' spacing={1}>
          {buttons.map((button) => {
            return (
              // @ts-ignore
              <Button
                key={button.buttonConent}
                variant={button.variant}
                color={button.color}
                onClick={() => {
                  button.callBack(doc);
                }}
                disabled={
                  button.disabled ? button.disabled(docHasChange) : false
                }
              >
                {button.buttonConent}
              </Button>
            );
          })}

          {err == null && <Alert severity='success'>Valid Document</Alert>}
          {!!err && <Alert severity='error'>{err.msgWithLoc}</Alert>}
        </Stack>
        <Stack
          width='40%'
          spacing={1}
          direction='row'
          alignContent='flex-start'
        >
          <span> Ln {cursorLine} </span>
          <span> Col {cursorColumn} </span>
        </Stack>
      </Stack>
    </div>
  );
}

function MongoDBModify(props: {
  doc: BSON.Document;
  verify: (doc: BSON.Document) => boolean;
  setOP: (doc: BSON.Document) => Promise<void>;
  buttonName: string;
}) {
  let { doc, setOP, verify, buttonName } = props;
  const [loading, setLoading] = useState(false);
  const [newDoc, setNewDoc] = useState(doc);
  const { showNotification } = useContext(NotificationContext);

  const handleModify = () => {
    if (!verify(newDoc)) {
      return;
    }
    setLoading(true);
    setOP(newDoc)
      .then(() => {
        showNotification('success', 'Modify OK');
        setLoading(false);
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'Modify failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      {loading && <CircularProgress />}
      <MongoBsonEditor
        doc={doc}
        handleChange={(e) => {
          setNewDoc(e);
        }}
        buttons={[
          {
            color: 'primary',
            callBack: (doc) => {
              setNewDoc(doc);
              handleModify();
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
}) {
  const {
    handleReload,
    handleFilter,
    handleOrderBy,
    orderBy,
    filter,
    viewModel,
    handleViewModel,
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
    <Stack direction='row' justifyContent='space-between'>
      <Stack direction='row' spacing={1} alignContent='flex-end'>
        <span>filter:</span>
        <Typography
          onClick={() => {
            setShowModifyFilter(true);
          }}
          style={{ cursor: 'pointer' }}
          variant='subtitle1'
        >
          {bsonValue2MongoBson(filter).format()}
        </Typography>

        {showModifyFilter && (
          <Modal
            open={showModifyFilter}
            onClose={() => {
              setShowModifyFilter(false);
            }}
            aria-labelledby='set filter'
          >
            <Box sx={modalStyle} width='600px'>
              <MongoDBModify
                buttonName='Set'
                verify={() => {
                  return true;
                }}
                setOP={async (doc) => {
                  handleFilter(doc);
                }}
                doc={filter}
              ></MongoDBModify>
            </Box>
          </Modal>
        )}

        <span>order by:</span>
        <Typography
          onClick={() => {
            setShowModifyOrderBy(true);
          }}
          style={{ cursor: 'pointer' }}
          variant='subtitle1'
        >
          {bsonValue2MongoBson(orderBy).format()}
        </Typography>

        {showModifyOrderBy && (
          <Modal
            open={showModifyOrderBy}
            onClose={() => {
              setShowModifyOrderBy(false);
            }}
            aria-labelledby='order by'
          >
            <Box sx={modalStyle} width='600px'>
              <MongoDBModify
                buttonName='Set'
                verify={() => {
                  return true;
                }}
                setOP={async (doc) => {
                  handleOrderBy(doc);
                }}
                doc={orderBy}
              ></MongoDBModify>
            </Box>
          </Modal>
        )}

        <Button
          variant='contained' // 可以是 'text', 'outlined', 或 'contained'
          color='primary' // 主题颜色
          onClick={doReload} // 点击事件处理
        >
          Reload
        </Button>
        {loading && <CircularProgress />}
      </Stack>
      <Stack direction='row' style={{ padding: '5px' }}>
        <ButtonGroup variant='contained' aria-label='Basic button group'>
          <IconButton
            onClick={() => {
              handleViewModel(MongoDataViewModel.List);
            }}
            disabled={viewModel == MongoDataViewModel.List}
            edge='start'
            color='inherit'
            aria-label='list'
          >
            <ListIcon />
          </IconButton>
          <IconButton
            disabled={viewModel == MongoDataViewModel.Object}
            onClick={() => {
              handleViewModel(MongoDataViewModel.Object);
            }}
            edge='start'
            color='inherit'
            aria-label='object'
          >
            <DataObjectIcon />
          </IconButton>
        </ButtonGroup>
      </Stack>
    </Stack>
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
  loadOne: (id: BSON.ObjectId) => Promise<BSON.Document>;
  modifyOP: (doc: BSON.Document) => Promise<void>;
  deleteOP: (ids: string[]) => Promise<void>;
  viewModel: MongoDataViewModel;
}) {
  const {
    reloadOPTableList,
    loadOne,
    modifyOP,
    realodOPDocs,
    deleteOP,
    viewModel: viewModelPrev,
  } = props;
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const [filter, setFilter] = useState<BSON.Document>({});
  const [orderBy, setOrderby] = useState<BSON.Document>({});
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(defaultPageSize);
  const [selectModifyDoc, setSelectModifyDoc] = useState<BSON.Document | null>(
    null
  );
  const [docs, setDocs] = useState<DocsAndCount | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [viewModel, setViewModel] = useState(viewModelPrev);
  const [selectIDs, setSelectIDs] = useState([]);

  const handleReload = () => {
    const handleReloadTableList = async () => {
      reloadOPTableList(filter, orderBy, page, pageSize)
        .then((resp) => {
          setResp(resp);
        })
        .catch((e) => {
          showNotification('error', 'fetch data failed', e);
        });
    };
    const handleReloadDocs = async () => {
      realodOPDocs(filter, orderBy, page, pageSize)
        .then((resp) => {
          setDocs(resp);
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

  const handleDetele = (ids: string[]) => {
    secondConfirm({
      title: 'Delete data',
      content: `are u sure delte those ${ids.join(',')} data`,
      confirmText: 'Delete',
    }).then((confirm) => {
      if (confirm) {
        deleteOP(ids)
          .then(() => {
            showNotification('success', 'Delete OK');
            handleReload();
          })
          .catch((e) => {
            showNotification('error', 'Delete failure', e);
          });
      }
    });
  };

  const handleLoadOne = async (id: string) => {
    try {
      let x = await loadOne(new BSON.ObjectId(id));
      return x;
    } catch (e) {
      showNotification('error', 'fetch data failed', e);
      throw e;
    }
  };

  useEffect(() => {
    handleReload();
  }, [page, pageSize, viewModel]);

  return (
    <Stack spacing={1}>
      <MongoTableListToolBar
        orderBy={orderBy}
        filter={filter}
        handleFilter={(e) => {
          setFilter(e);
        }}
        handleOrderBy={(e) => {
          setOrderby(e);
        }}
        handleReload={async () => {
          handleReload();
        }}
        handleViewModel={(e) => {
          setViewModel(e);
        }}
        viewModel={viewModel}
      ></MongoTableListToolBar>

      {/* // datagrid modeal */}
      {resp && viewModel == MongoDataViewModel.List && (
        <Stack spacing={1}>
          <Stack direction='row'>
            <Button
              variant='contained'
              color='error'
              disabled={selectIDs.length == 0}
              onClick={() => {
                handleDetele(selectIDs);
              }}
            >
              <DeleteForeverIcon></DeleteForeverIcon>
            </Button>
          </Stack>
          <DataGrid
            getRowId={(e) => {
              return e._id; // in this case object been convert to hexstring
            }}
            rowHeight={datagridRowHeight}
            columnHeaderHeight={datagridHeaderHeight}
            checkboxSelection
            rowCount={resp.total}
            columns={resp.data.fields}
            rows={resp.data.data}
            pageSizeOptions={pageSizeOptions}
            disableColumnFilter
            paginationMode='server'
            paginationMeta={{ hasNextPage: true }}
            paginationModel={{ page: page, pageSize: pageSize }}
            onPaginationModelChange={(m) => {
              setPage(m.page);
              setPageSize(m.pageSize);
            }}
            onRowDoubleClick={(e) => {
              handleLoadOne(e.id as string).then((doc) => {
                setSelectModifyDoc(doc);
              });
            }}
            sortingMode='server'
            onSortModelChange={(e) => {
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
            }}
            onRowSelectionModelChange={(e) => {
              setSelectIDs(e as string[]);
            }}
          ></DataGrid>
        </Stack>
      )}
      {selectModifyDoc && (
        <Modal
          open={!!selectModifyDoc}
          onClose={() => {
            setSelectModifyDoc(null);
          }}
          aria-labelledby='modify set'
        >
          <Box sx={modalStyle} width='600px'>
            <MongoDBModify
              verify={() => {
                return true;
              }}
              setOP={async (doc) => {
                try {
                  await modifyOP(doc);
                } catch (e) {
                  throw e;
                }
                handleReload();
              }}
              doc={selectModifyDoc}
              buttonName='Modify'
            ></MongoDBModify>
          </Box>
        </Modal>
      )}
      {/* // docs modal  */}
      {docs && viewModel == MongoDataViewModel.Object && (
        <Stack spacing={1} style={{ paddingRight: '40px' }}>
          {docs.docs.map((doc) => {
            return (
              <MongoBsonEditor
                key={(doc._id as BSON.ObjectId).toHexString()}
                doc={doc}
                handleChange={() => {
                  // here need do nothing
                }}
                buttons={[
                  {
                    buttonConent: 'set',
                    variant: 'contained',
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
                    buttonConent: 'delte',
                    color: 'error',
                    variant: 'outlined',
                    callBack: (_) => {
                      handleDetele([(doc._id as ObjectId).toHexString()]);
                    },
                  },
                ]}
              ></MongoBsonEditor>
            );
          })}
          {/* // here is pagination */}
          <Stack direction='row' spacing={1} alignItems='flex-end'>
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
            <SelectPageSize
              pageSize={pageSize}
              onChange={(size) => {
                setPage(size);
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
  const [doc, setDoc] = useState({});
  const handleAdd = () => {
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
        doc={doc}
        handleChange={(doc) => {
          setDoc(doc);
        }}
        buttons={[
          {
            buttonConent: 'Add',
            variant: 'contained',
            disabled: (changed) => {
              return !changed;
            },
            callBack: () => {
              handleAdd();
            },
          },
        ]}
      ></MongoBsonEditor>
    </Stack>
  );
}

export { MongoBsonEditor, MongoTable, MongoTableAdd };
