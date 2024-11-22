import Editor from '@monaco-editor/react';
import {
  Alert,
  Box,
  Button,
  ButtonGroup,
  CircularProgress,
  IconButton,
  Modal,
  Stack,
  Typography,
} from '@mui/material';

import ListIcon from '@mui/icons-material/List';

import { loader } from '@monaco-editor/react';
import { CancellationToken, editor, languages, Position } from 'monaco-editor';

import {
  bsonValue2MongoBson,
  formatMongoBsonStr,
  MongoBsonBinaryAllSubType,
  MongoBsonError,
  MongoBsonObject,
  parseMongoBson,
} from './MongoBsonCompiler';
import { useContext, useEffect, useState } from 'react';
import { BSON } from 'bson';
import types from './ApiStruct';
import { NotificationContext } from './Notification';
import { DataGrid } from '@mui/x-data-grid';
import { modalStyle } from './common';

import DataObjectIcon from '@mui/icons-material/DataObject';
const mongoBsonKeywords =
  /\b(as|int|long|date|timestamp|decimal|binary|generic|function|byte_array|uuid|md5|encrypted|column|sensitive|vector|user_define|true|false|null|undefined|code)\b/;
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

function MongoBsonEditor(props: {
  doc: BSON.Document;
  maxHeight: number;
  handleChange: (text: string) => void;
}) {
  let { doc, maxHeight, handleChange } = props;
  let content = bsonValue2MongoBson(doc).format();

  const [heigth, setHeigth] = useState('200px');
  const [cursorLine, setCursorLine] = useState(1);
  const [cursorColumn, setCursorColumn] = useState(1);
  const [err, setErr] = useState<MongoBsonError | null>(null);
  const [newText, setNewText] = useState(content);

  useEffect(() => {
    try {
      parseMongoBson(newText).compile();
    } catch (e) {
      setErr(e);
      console.log('@@@@@@@@@@@@@@@@', e);
      return;
    }
    setErr(null);
  }, [newText]);

  const handleEditorDidMount = (ce: editor.IStandaloneCodeEditor) => {
    ce.onDidChangeModelContent((_event) => {
      let text = ce.getValue();
      handleChange(text);
      setNewText(text);
    });
    ce.onDidChangeCursorPosition((event) => {
      setCursorLine(event.position.lineNumber);
      setCursorColumn(event.position.column);
    });

    // TODO怎么实现适应性高度呢
    // ce.onDidChangeModelContent(() => {
    //   const lineCount = ce.getModel().getLineCount();
    //   const lineHeight = ce.getOption(editor.EditorOption.lineHeight);
    //   const padding = 20;
    //   let newHeight = lineCount * lineHeight + padding;
    //   if (newHeight > maxHeight) {
    //     newHeight = maxHeight;
    //   }
    //   setHeigth(`${newHeight}px`);
    // });
  };

  return (
    <Stack spacing={1}>
      <Box style={{ height: heigth }}>
        <Editor
          onMount={handleEditorDidMount}
          language='mongoBson'
          value={content}
        ></Editor>
      </Box>
      <Stack direction='row' justifyContent='space-between'>
        <Stack direction='row' alignContent='flex-start'>
          {err == null && <Alert severity='success'>OK</Alert>}
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
    </Stack>
  );
}

function MongoDBModify(props: {
  doc: BSON.Document;
  handleClose: () => void;
  verify: (doc: BSON.Document) => boolean;
  setOP: (doc: BSON.Document) => Promise<void>;
  buttonName: string;
}) {
  let { doc, handleClose, setOP, verify, buttonName } = props;
  const [loading, setLoading] = useState(false);
  const [text, setNewText] = useState('');
  const { showNotification } = useContext(NotificationContext);
  useEffect(() => {
    let t = bsonValue2MongoBson(doc).format(2);
    setNewText(t);
  }, []);

  const handleModify = () => {
    let obj: MongoBsonObject;
    try {
      obj = parseMongoBson(text);
    } catch (e) {
      showNotification('error', 'parse bson failed', e);
      return;
    }
    let doc: BSON.Document;
    try {
      doc = obj.compile();
    } catch (e) {
      showNotification('error', 'Compile failed', e);
      return;
    }
    if (!verify(doc)) {
      return;
    }
    setOP(doc)
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
      <MongoBsonEditor
        doc={doc}
        maxHeight={500}
        handleChange={(e) => {
          setNewText(e);
        }}
      ></MongoBsonEditor>
      <Stack direction='row' spacing={1}>
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
        </Button>
        <Button color='primary' variant='contained' onClick={handleModify}>
          {buttonName}
        </Button>
        {loading && (
          <Box sx={{ display: 'flex' }}>
            <CircularProgress />
          </Box>
        )}
      </Stack>
    </Stack>
  );
}

function MongoTableListToolBar(props: {
  handleReload: () => Promise<void>;
  handleFilter: (doc: BSON.Document) => void;
  handleOrderBy: (doc: BSON.Document) => void;
  orderByPrev: BSON.Document;
  filterPrev: BSON.Document;
  viewModelPrev: MongoDataViewModel;
  handleViewModel: (doc: MongoDataViewModel) => void;
}) {
  let {
    handleReload,
    handleFilter,
    handleOrderBy,
    orderByPrev,
    filterPrev,
    viewModelPrev,
    handleViewModel,
  } = props;

  const [filter, setFilter] = useState<BSON.Document>(filterPrev);
  const [orderBy, setOrderby] = useState<BSON.Document>(orderByPrev);
  const [orderByText, setOrderByText] = useState('');
  const [filterText, setFilterText] = useState('');
  const [showModifyFilter, setShowModifyFilter] = useState(false);
  const [showModifyOrderBy, setShowModifyOrderBy] = useState(false);
  const [viewModel, setViewModel] = useState(viewModelPrev);

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

  useEffect(() => {
    let x = bsonValue2MongoBson(orderBy).format(2);
    setOrderByText(x);
  }, [orderBy]);

  useEffect(() => {
    let x = bsonValue2MongoBson(filter).format(2);
    setFilterText(x);
  }, [filter]);

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
          {filterText}
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
                  setFilter(doc);
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

        <span>order by:</span>
        <Typography
          onClick={() => {
            setShowModifyOrderBy(true);
          }}
          style={{ cursor: 'pointer' }}
          variant='subtitle1'
        >
          {orderByText}
        </Typography>

        {showModifyOrderBy && (
          <Modal
            open={showModifyOrderBy}
            onClose={() => {
              setShowModifyOrderBy(false);
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
                  setOrderby(doc);
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
          variant='contained' // 可以是 'text', 'outlined', 或 'contained'
          color='primary' // 主题颜色
          onClick={doReload} // 点击事件处理
        >
          Reload
        </Button>
        {loading && (
          <Box sx={{ display: 'flex' }}>
            <CircularProgress />
          </Box>
        )}
      </Stack>
      <Stack direction='row'>
        <ButtonGroup variant='contained' aria-label='Basic button group'>
          <IconButton
            onClick={() => {
              setViewModel(MongoDataViewModel.List);
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
              setViewModel(MongoDataViewModel.Object);
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

enum MongoDataViewModel {
  List,
  Object,
}

function MongoTableList(props: {
  reloadOP: (
    filter: BSON.Document,
    orderBy: BSON.Document,
    page: number,
    pageSize: number
  ) => Promise<types.MuiDataTableListResp>;
  loadOne: (id: BSON.ObjectId) => Promise<BSON.Document>;
  modifyOP: (doc: BSON.Document) => Promise<void>;
}) {
  const { reloadOP, loadOne, modifyOP } = props;
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const [filter, setFilter] = useState<BSON.Document>({});
  const [orderBy, setOrderby] = useState<BSON.Document>({});
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(10);
  const [selectDoc, setSelectDoc] = useState<BSON.Document | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [viewModel, setViewModel] = useState(MongoDataViewModel.List);

  const handleReloadTableList = async () => {
    reloadOP(filter, orderBy, page, pageSize)
      .then((resp) => {
        setResp(resp);
      })
      .catch((e) => {
        showNotification('error', 'fetch data failed', e);
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
    handleReloadTableList();
  }, [page, pageSize]);

  useEffect(() => {}, [orderBy]);
  return (
    <Stack spacing={1}>
      <MongoTableListToolBar
        orderByPrev={orderBy}
        filterPrev={filter}
        handleFilter={(e) => {
          setFilter(e);
        }}
        handleOrderBy={(e) => {
          setOrderby(e);
        }}
        handleReload={async () => {
          handleReloadTableList();
        }}
        handleViewModel={(e) => {
          setViewModel(e);
        }}
        viewModelPrev={viewModel}
      ></MongoTableListToolBar>
      {resp && (
        <DataGrid
          getRowId={(e) => {
            return e._id;
          }}
          checkboxSelection
          rowCount={resp.total}
          columns={resp.data.fields}
          rows={resp.data.data}
          pageSizeOptions={[10]}
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
              setSelectDoc(doc);
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
        ></DataGrid>
      )}
      {selectDoc && (
        <Modal
          open={!!selectDoc}
          onClose={() => {
            setSelectDoc(null);
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
                handleReloadTableList();
              }}
              handleClose={() => {
                setSelectDoc(null);
              }}
              doc={selectDoc}
              buttonName='Modify'
            ></MongoDBModify>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

export { MongoBsonEditor, MongoTableList };
