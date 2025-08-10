import {
  Button,
  FormControlLabel,
  IconButton,
  ListItemIcon,
  ListItemText,
  MenuItem,
  Radio,
  RadioGroup,
  Stack,
  SxProps,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import SaveIcon from '@mui/icons-material/Save';
import {
  languages,
  Position as EditorPositon,
  editor,
  Position,
  CancellationToken,
} from 'monaco-editor';
import { types } from './ApiStruct';

import { BSON } from 'bson';
import {
  createContext,
  CSSProperties,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { Editor, loader, Monaco, OnMount } from '@monaco-editor/react';
import { TabContent } from './Tabs';
import { invoke } from '@tauri-apps/api/core';
import { NotificationContext } from './Notification';
import InfoIcon from '@mui/icons-material/Info';
import AttachFileIcon from '@mui/icons-material/AttachFile';
const modalStyle: SxProps = {
  position: 'absolute',
  top: '50%',
  left: '50%',
  transform: 'translate(-50%, -50%)',
  bgcolor: 'background.paper',
  border: '2px solid skyblue',
  boxShadow: 24,
  margin: 0,
  borderRadius: '10px',
  p: 1,
  maxHeight: '85vh',
  overflowY: 'auto',
};

import {
  cassandraKeywords,
  clickhouseKeyWords,
  cockroachKeywords,
  duckdbKeyWords,
  firebirdKeywords,
  mariadbKeyWords,
  mssqlKeywords,
  mysqlKeywords,
  oracleKeywords,
  postgresKeywords,
  redisKeywords,
  sqliteKeywords,
  tidbKeywords,
} from './keywords';
import {
  open,
  OpenDialogOptions,
  save,
  SaveDialogOptions,
} from '@tauri-apps/plugin-dialog';
import { readFile, writeFile } from '@tauri-apps/plugin-fs';
import {
  appStore,
  AppStoreKey,
  backendCompletionItemToEditor,
  DataSourceKind,
  saveAndSetHeight,
  secondConfirm,
  TabsSetContext,
} from './helper';
import { MetaValuesView } from './CommonComp';

const getNamedConfig = async (
  name: string
): Promise<types.DBConnectionConfig> => {
  let x = await appStore.get<types.DBConnectionConfig[]>(
    AppStoreKey.ConnectionConfig
  );
  if (!x) {
    return undefined;
  }
  return x.find((x) => x.name == name);
};

class DocsAndCount {
  constructor(public docs: BSON.Document[], public total: number) {}
}

function EditorBar(props: {
  left?: any;
  pos: EditorPositon;
  style?: CSSProperties | undefined;
}) {
  let { left, pos, style } = props;
  return (
    <Stack
      justifyContent="space-between"
      direction="row"
      style={style}
      alignItems="center"
    >
      <div style={{ width: '40%' }}> {left}</div>
      <Stack width="40%" direction="row" spacing={1}>
        <span>Ln {pos ? pos.lineNumber : ''}</span>
        <span>Column {pos ? pos.column : ''}</span>
      </Stack>
    </Stack>
  );
}

function CreateDatabase({ uuid, handleClose, kind }) {
  const { showNotification } = useContext(NotificationContext);
  const [database, setDatabase] = useState('');
  const databaseDom = useRef(null);

  const handleCreate = () => {
    if (!database) {
      databaseDom.current.focus();
      showNotification('error', 'database name is empty');
      return;
    }

    let extra = null;
    invoke('create_database', { name: uuid, database, extra })
      .then(() => {
        showNotification('success', 'Create OK');
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Create Failure', e);
      });
  };
  let createButtonName;
  switch (kind) {
    default:
      createButtonName = 'create';
      break;
  }
  return (
    <Stack spacing={1}>
      <Stack>
        <TextField
          size="small"
          inputRef={databaseDom}
          value={database}
          onChange={(e) => {
            setDatabase(e.target.value);
          }}
          label="database name"
        ></TextField>
      </Stack>
      <Stack direction="row">
        <Button
          onClick={() => {
            handleCreate();
          }}
        >
          {createButtonName}
        </Button>
      </Stack>
    </Stack>
  );
}

function CreateSchema(props: {
  uuid;
  handleClose;
  handleReload;
  kind: DataSourceKind;
  schemas: string[];
}) {
  const { uuid, handleClose, kind, schemas, handleReload } = props;
  const schemaSet = new Set(schemas);
  const { showNotification } = useContext(NotificationContext);
  const [schema, setSchema] = useState('');
  const schemaDom = useRef(null);
  const replicationStrateges = ['SimpleStrategy', 'NetworkTopologyStrategy'];
  const [replicationStrategy, setReplicationStrategy] = useState(
    replicationStrateges[0]
  );
  const [replicationFactor, setReplicationFactor] = useState('3');
  const handleCreate = () => {
    if (!schema) {
      schemaDom.current.focus();
      showNotification('error', 'Schema is empty');
      return;
    }
    if (schemaSet.has(schema)) {
      showNotification('error', `'${schema}' already exists.`);
      return;
    }
    let extra = null;
    switch (kind) {
      case DataSourceKind.Cassandra:
        extra = {
          extra: 'cassandra',
          replication_strategy: replicationStrategy,
          replication_factor: parseInt(replicationFactor),
        };
        break;
    }
    invoke('create_schema', { name: uuid, schema, extra })
      .then(() => {
        showNotification('success', 'Create OK');
        handleClose();
        handleReload();
      })
      .catch((e) => {
        showNotification('error', 'Create Failure', e);
      });
  };

  if (kind === DataSourceKind.Cassandra) {
    //
    return (
      <Stack spacing={1}>
        <RadioGroup
          row
          onChange={(e) => {
            setReplicationStrategy(e.target.value);
          }}
          value={replicationStrategy}
        >
          {replicationStrateges.map((e) => (
            <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
          ))}
        </RadioGroup>
        <TextField
          type="number"
          size="small"
          value={replicationFactor}
          onChange={(e) => {
            setReplicationFactor(e.target.value);
          }}
          label="replication factor"
        ></TextField>
        <TextField
          size="small"
          value={schema}
          onChange={(e) => {
            setSchema(e.target.value);
          }}
          inputRef={schemaDom}
          label="name"
        ></TextField>
        <Stack direction="row">
          <Button
            onClick={() => {
              handleCreate();
            }}
          >
            create
          </Button>
        </Stack>
      </Stack>
    );
  }
  if (kind === DataSourceKind.Redis) {
    // redis have special create
    const range = [];
    for (let i = 0; i < 16; i++) {
      range.push('db' + i);
    }
    return (
      <Stack spacing={1}>
        <Stack direction="row" style={{ flexWrap: 'wrap' }}>
          {range
            .filter((e) => !schemaSet.has(e))
            .map((e) => (
              <Button
                key={e}
                onClick={() => {
                  setSchema(e);
                }}
              >
                {e}
              </Button>
            ))}
        </Stack>
        <Stack>
          <TextField
            size="small"
            value={schema}
            onChange={(e) => {
              setSchema(e.target.value);
            }}
            inputRef={schemaDom}
            label="name"
          ></TextField>
        </Stack>
        <Stack direction="row">
          <Button
            onClick={() => {
              handleCreate();
            }}
          >
            create
          </Button>
        </Stack>
      </Stack>
    );
  }

  return (
    <Stack spacing={1}>
      <Stack>
        <TextField
          size="small"
          value={schema}
          onChange={(e) => {
            setSchema(e.target.value);
          }}
          label="name"
          inputRef={schemaDom}
        ></TextField>
      </Stack>
      <Stack direction="row">
        <Button
          onClick={() => {
            handleCreate();
          }}
        >
          create
        </Button>
      </Stack>
    </Stack>
  );
}

async function dropDatabase(props: {
  uuid: string;
  database;
  showNotification;
  okCallback?;
}) {
  const { uuid, database, showNotification, okCallback } = props;

  const confirm = await secondConfirm({
    title: `drop database ${database}`,
    confirmText: 'Drop',
  });
  if (!confirm) {
    return;
  }
  invoke('drop_database', { name: uuid, database })
    .then(() => {
      showNotification('success', 'Drop OK');
      if (okCallback) {
        okCallback();
      }
    })
    .catch((e) => {
      showNotification('error', 'Drop failure', e);
    });
}

async function dropSchema({ uuid, schema, showNotification, okCallback }) {
  const confirm = await secondConfirm({
    title: `drop ${schema}`,
    confirmText: 'Drop',
  });
  if (!confirm) {
    return;
  }
  invoke('drop_schema', { name: uuid, schema })
    .then(() => {
      showNotification('success', 'Drop OK');
      okCallback();
    })
    .catch((e) => {
      showNotification('error', 'Drop failure', e);
    });
}

function SimpleResizeEditor(props: {
  language: string;
  value?: string;
  onMount?: OnMount;
  barLeft?: ReactNode;
  wordWrap?: string;
  readOnly?: boolean;
  heightName: string;
}) {
  let { language, value, onMount, barLeft, wordWrap, readOnly, heightName } =
    props;
  const [pos, setPos] = useState<Position>(null);
  const setting = useContext(SettingContext);

  const handleDidMount = useCallback(
    (editor: editor.IStandaloneCodeEditor, monaco: Monaco) => {
      editor.onDidChangeCursorPosition(() => {
        setPos(editor.getPosition());
      });
      if (onMount) {
        onMount(editor, monaco);
      }
    },
    [onMount]
  );

  const options = useMemo((): editor.IStandaloneEditorConstructionOptions => {
    return {
      fontSize: setting.editorFontSize,
      //@ts-ignore
      wordWrap: wordWrap,
      readOnly,

      ...setting.editorOption,
    };
  }, [setting]);

  const divDom = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let x1 = saveAndSetHeight(divDom.current, heightName);
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
          width="99.5%"
          onMount={handleDidMount}
          language={language}
          value={value}
          theme={setting.editorTheme()}
          options={options}
        ></Editor>
      </div>
      <EditorBar left={barLeft} pos={pos}></EditorBar>
    </div>
  );
}

const msgTimeout = 10000;

function tabsHasId(tabs: TabContent[], id: string): number | null {
  for (let i = 0; i < tabs.length; i++) {
    let tab = tabs[i];
    if (tab.id === id) {
      return i;
    }
  }
  return null;
}

interface DefAndMetas {
  def: string;
  metas: types.MetaValue[];
}
const progressBarSize = '30px';
const progressBarSizeSmall = '20px';

function MenuConnectionInfo({ uuid }) {
  const { tabs, setSelectTab, setTabs } = useContext(TabsSetContext);

  const handleInfo = async () => {
    let id = uuid + '-info';
    let index = tabsHasId(tabs, id);
    if (typeof index === 'number') {
      setSelectTab(index);
    } else {
      setTabs((e) => {
        setSelectTab(tabs.length);
        return [
          ...e,
          new TabContent(
            id,
            uuid,
            <ConnConfigInfo uuid={uuid}></ConnConfigInfo>,
            <InfoIcon></InfoIcon>
          ),
        ];
      });
    }
  };
  return (
    <MenuItem onClick={handleInfo}>
      <ListItemIcon>
        <InfoIcon></InfoIcon>
      </ListItemIcon>
      <ListItemText> Info </ListItemText>
    </MenuItem>
  );
}

class UserSettings {
  theme: string = 'light';
  pageSize: number = 100;
  editorFontSize: number = 16;
  rootFontSize: number = 15;
  datagridRowHeight: number = 24;
  datagridHeaderHeight: number = 30;
  editorOption: editor.IStandaloneEditorConstructionOptions = {};
  editorTheme(): string {
    return this.theme === 'dark' ? 'vs-dark' : 'light';
  }
  constructor(
    theme?,
    pageSize?,
    editorFontSize?,
    rootFontSize?,
    datagridRowHeight?,
    datagridHeaderHeight?,
    editorOption?
  ) {
    if (theme) {
      this.theme = theme;
    }
    if (pageSize) {
      this.pageSize = pageSize;
    }
    if (editorFontSize) {
      this.editorFontSize = editorFontSize;
    }
    if (rootFontSize) {
      this.rootFontSize = rootFontSize;
    }
    if (datagridHeaderHeight) {
      this.datagridHeaderHeight = datagridHeaderHeight;
    }
    if (datagridRowHeight) {
      this.datagridRowHeight = datagridRowHeight;
    }
    if (editorOption) {
      this.editorOption = editorOption;
    }
  }
}

const SettingContext = createContext<UserSettings>(null);

const OpenedContext = createContext<{ opened: boolean; setOpened }>(null);

function ConnConfigInfo(props: { uuid }) {
  let { uuid } = props;
  const { showNotification } = useContext(NotificationContext);
  const [metas, setMetas] = useState<types.MetaValue[]>([]);
  const handleInfo = async () => {
    try {
      let metas = await invoke<types.MetaValue[]>('server_metas', {
        name: uuid,
      });
      let config = await getNamedConfig(uuid);
      let configMetas = [];
      switch (config.conn_type) {
        case DataSourceKind.Mysql:
        case DataSourceKind.MariaDB:
        case DataSourceKind.TiDB:
        case DataSourceKind.Postgres:
        case DataSourceKind.Cockroach:
        case DataSourceKind.Redis:
        case DataSourceKind.Mongo:
        case DataSourceKind.Cassandra:
        case DataSourceKind.Oracle:
        case DataSourceKind.ClickHouse:
        case DataSourceKind.FireBird:
        case DataSourceKind.MsSql:
          configMetas = [
            { name: 'host', value: config.host } as types.MetaValue,
            { name: 'port', value: config.port + '' } as types.MetaValue,
            { name: 'user', value: config.username } as types.MetaValue,
          ];
          break;
        case DataSourceKind.LibSql:
          if (config.source === 'local') {
            configMetas = [
              { name: 'host', value: config.path } as types.MetaValue,
            ];
          } else if (config.source == 'remote') {
            configMetas = [
              { name: 'url', value: config.url } as types.MetaValue,
              { name: 'path', value: config.path } as types.MetaValue,
            ];
          }
          break;
        case DataSourceKind.Sqlite:
          configMetas = [
            { name: 'path', value: config.path } as types.MetaValue,
          ];
          break;
      }
      setMetas([...configMetas, ...metas]);
    } catch (e) {
      showNotification('error', 'call server_metas failed', e);
      console.log(e);
    }
  };

  useEffect(() => {
    handleInfo();
  }, []);

  return (
    <Stack spacing={1}>
      <Stack direction="row">
        <Button variant="contained" color="primary" onClick={handleInfo}>
          reload
        </Button>
      </Stack>

      <MetaValuesView values={metas}></MetaValuesView>
    </Stack>
  );
}
function reloadOK(showNotification) {
  showNotification('success', 'data loaded.');
}
const registeredLanguage = new Set<string>();

function makeAutoComplete(
  uuid
): (sql: string, pos: Position) => Promise<languages.CompletionList> {
  const autoCompletion = async (
    sql: string,
    pos: Position
  ): Promise<languages.CompletionList> => {
    try {
      let completionItem = await invoke<types.CompletionItem[]>(
        'auto_completion',
        {
          name: uuid,
          sql,
          line: pos.lineNumber,
          col: pos.column,
        }
      );
      let suggestions = backendCompletionItemToEditor(completionItem);
      return suggestions;
    } catch (e) {
      console.log('completion server error:', e);
      throw e;
    }
  };
  return autoCompletion;
}

// uuid as language identifier.
function initMonacoLanague(uuid: string, kind: DataSourceKind) {
  if (registeredLanguage.has(uuid)) {
    return;
  }
  registeredLanguage.add(uuid);
  const autoCompletion = makeAutoComplete(uuid);
  const format = async (sql: string, _opts: languages.FormattingOptions) => {
    try {
      let text = await invoke<string>('format_sql', {
        name: uuid,
        sql,
      });
      return text;
    } catch (e) {
      console.log('completion server error:', e);
      throw e;
    }
  };
  function initLanguageXXX(
    uuid: string,
    Ilanguage: languages.IMonarchLanguage,
    autoCompletion: (
      text: string,
      pos: Position
    ) => Promise<languages.CompletionList>,
    format?: (
      text: string,
      options: languages.FormattingOptions
    ) => Promise<string>
  ) {
    loader
      .init()
      .then((monaco) => {
        monaco.languages.register({ id: uuid });
        monaco.languages.setMonarchTokensProvider(uuid, Ilanguage);
        if (format) {
          monaco.languages.registerDocumentFormattingEditProvider(uuid, {
            provideDocumentFormattingEdits(model, options, _token) {
              return format(model.getValue(), options).then((text) => [
                { range: model.getFullModelRange(), text },
              ]);
            },
          });
        }

        monaco.languages.registerCompletionItemProvider(uuid, {
          provideCompletionItems: (
            model: editor.ITextModel,
            position: Position,
            _context: any,
            _token: CancellationToken
          ) => {
            let text = model.getValue();
            return autoCompletion(text, position);
          },
        });
      })
      .catch((e) => {
        console.log('init monaco language error:', e);
      });
  }

  switch (kind) {
    case DataSourceKind.MariaDB:
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [mariadbKeyWords, 'keyword'],
              [/[\$_a-z][\$_a-z0-9]*/i, 'identifier'],
              [/\'(?:\\.|[^\\\'])*\'/, 'string'],
              [/\/\*[^]*?\*\//g, 'comment'], // not working why
              [/(--[^\n]*|#[^\n]*)/g, 'comment'],
            ],
          },
          brackets: [
            { open: '{', close: '}', token: 'delimiter.curly' },
            { open: '[', close: ']', token: 'delimiter.square' },
          ],
        } as languages.IMonarchLanguage,
        autoCompletion,
        format
      );
      break;
    case DataSourceKind.Mysql:
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [mysqlKeywords, 'keyword'],
              [/[\$_a-z][\$_a-z0-9]*/i, 'identifier'],
              [/\'(?:\\.|[^\\\'])*\'/, 'string'],
              [/\/\*[^]*?\*\//g, 'comment'], // not working why
              [/(--[^\n]*|#[^\n]*)/g, 'comment'],
            ],
          },
          brackets: [
            { open: '{', close: '}', token: 'delimiter.curly' },
            { open: '[', close: ']', token: 'delimiter.square' },
          ],
        } as languages.IMonarchLanguage,
        autoCompletion,
        format
      );
      break;
    case DataSourceKind.ClickHouse:
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [clickhouseKeyWords, 'keyword'],
              [/[\$_a-z][\$_a-z0-9]*/i, 'identifier'],
              [/\'(?:\\.|[^\\\'])*\'/, 'string'],
              [/\/\*[^]*?\*\//g, 'comment'], // not working why
              [/(--[^\n]*|#[^\n]*)/g, 'comment'],
            ],
          },
          brackets: [
            { open: '(', close: ')', token: 'delimiter.parenthesis' },
            { open: '[', close: ']', token: 'delimiter.square' },
          ],
        } as languages.IMonarchLanguage,
        autoCompletion,
        format
      );
      break;

    case DataSourceKind.TiDB:
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [tidbKeywords, 'keyword'],
              [/[\$_a-z][\$_a-z0-9]*/i, 'identifier'],
              [/\'(?:\\.|[^\\\'])*\'/, 'string'],
              [/\/\*[^]*?\*\//g, 'comment'], // not working why
              [/(--[^\n]*|#[^\n]*)/g, 'comment'],
            ],
          },
          brackets: [
            { open: '(', close: ')', token: 'delimiter.parenthesis' },
            { open: '[', close: ']', token: 'delimiter.square' },
          ],
        } as languages.IMonarchLanguage,
        autoCompletion,
        format
      );
      break;
    case DataSourceKind.Sqlite:
    case DataSourceKind.LibSql:
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [sqliteKeywords, 'keyword'],
              [/[_a-z][_a-z0-9]*/i, 'identifier'],
              [/\'(?:\\.|[^\\\'])*\'/, 'string'],
              [/\/\*[^]*?\*\//g, 'comment'], // not working why
              [/(--[^\n]*|#[^\n]*)/g, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,
        autoCompletion,
        format
      );
      break;
    case DataSourceKind.FireBird:
      // TODO keywords
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [firebirdKeywords, 'keyword'],
              [/[_a-z][_a-z0-9]*/i, 'identifier'],
              [/\'(?:\\.|[^\\\'])*\'/, 'string'],
              [/\/\*[^]*?\*\//g, 'comment'], // not working why
              [/(--[^\n]*|#[^\n]*)/g, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,
        autoCompletion,
        format
      );
      break;
    case DataSourceKind.DuckDB:
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [duckdbKeyWords, 'keyword'],
              [/[_a-z\$][_a-z0-9\$]*/i, 'identifier'],
              [/\"(?:\\.|[^\\\"])*\"/, 'string'],
              [/\'(?:\\.|[^\\\'])*\'/, 'string'],
              [/\`(?:\\.|[^\\\`])*\`/, 'string'],
              [/\/\*[^]*?\*\//g, 'comment'], // not working why
              [/(--[^\n]*|#[^\n]*)/g, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,
        autoCompletion,
        format
      );
      break;

    case DataSourceKind.Postgres:
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [postgresKeywords, 'keyword'],
              [/[_a-z][_a-z0-9]*/i, 'identifier'],
              [/\'(?:\\.|[^\'\'])*\'/, 'string'],
              [/\/\*[^]*?\*\//g, 'comment'], // not working why
              [/(--[^\n]*|#[^\n]*)/g, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,
        autoCompletion,
        format
      );
      break;
    case DataSourceKind.Cockroach:
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [cockroachKeywords, 'keyword'],
              [/[_a-z][_a-z0-9]*/i, 'identifier'],
              [/\"(?:\\.|[^\"\"])*\"/, 'string'],
              [/\'(?:\\.|[^\'\'])*\'/, 'string'],
              [/\/\*[^]*?\*\//g, 'comment'], // not working why
              [/(--[^\n]*|#[^\n]*)/g, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,
        autoCompletion,
        format
      );
      break;
    case DataSourceKind.MsSql:
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [mssqlKeywords, 'keyword'],
              [/[_a-z][_a-z0-9]*/i, 'identifier'],
              [/\'(?:\\.|[^\'\'])*\'/, 'string'],
              [/\"(?:\\.|[^\\\"])*\"/, 'string'],
              [/\/\*[^]*?\*\//g, 'comment'], // not working why
              [/(--[^\n]*|#[^\n]*)/g, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,
        autoCompletion,
        format
      );
      break;
    case DataSourceKind.Redis:
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: { root: [[redisKeywords, 'keyword']] },
        } as languages.IMonarchLanguage,
        autoCompletion
      );
      break;
    case DataSourceKind.Mongo:
      // here is no-op mongo use frontend to execute query
      break;
    case DataSourceKind.Oracle:
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [oracleKeywords, 'keyword'],
              [/[_a-z][_a-z0-9]*/i, 'identifier'],
              [/\'(?:\\.|[^\'\'])*\'/, 'string'],
              [/\"(?:\\.|[^\\\"])*\"/, 'string'],
              [/\/\*[^]*?\*\//g, 'comment'], // not working why
              [/(--[^\n]*|#[^\n]*)/g, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,
        autoCompletion,
        format
      );
      break;
    case DataSourceKind.Cassandra:
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [cassandraKeywords, 'keyword'],
              [/[_a-z][_a-z0-9]*/i, 'identifier'],
              [/\'(?:\\.|[^\'\'])*\'/, 'string'],
              [/\"(?:\\.|[^\\\"])*\"/, 'string'],
              [/\/\*[^]*?\*\//g, 'comment'], // not working why
              [/(--[^\n]*|#[^\n]*)/g, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,
        autoCompletion,
        format
      );
      break;
    default:
      throw `impl this ${kind}`;
  }
}

interface IQueryConn {
  queryOP: (text: string) => Promise<any[]>;
  executeOP: (text: string) => Promise<any>;
  loadQueryLog: () => Promise<types.QueryLogEvent[]>;
  reconnect: () => Promise<void>;
  exportQueryResult: (
    sql: string,
    kind: types.ExportKind,
    file: string
  ) => Promise<void>;
  isDangerSql: (sql: string) => Promise<boolean>;
  destroy: () => void;
  setAutoCommit: (boolean) => Promise<void>;
  getAutoCommit: () => Promise<boolean>;
  commit: () => Promise<void>;
  rollback: () => Promise<void>;
  supportTransaction: () => boolean;
  txExecuteCounts: () => Promise<number>;
}

class QueryConn implements IQueryConn {
  queryId: string;
  uuid: string;
  kind: DataSourceKind;
  constructor(uuid, queryId: string, kind: DataSourceKind) {
    this.queryId = queryId;
    this.uuid = uuid;
    this.kind = kind;
  }

  txExecuteCounts(): Promise<number> {
    return invoke<number>('query_tx_execute_counts', {
      name: this.uuid,
      queryId: this.queryId,
    });
  }

  supportTransaction(): boolean {
    return [
      DataSourceKind.Oracle,
      DataSourceKind.FireBird,
      DataSourceKind.Sqlite,
      DataSourceKind.LibSql,
      DataSourceKind.Mysql,
      DataSourceKind.MariaDB,
      DataSourceKind.TiDB,
      DataSourceKind.Postgres,
      DataSourceKind.Cockroach,
      DataSourceKind.MsSql,
    ].includes(this.kind);
  }

  exportQueryResult(
    sql: string,
    kind: types.ExportKind,
    file: string
  ): Promise<void> {
    return invoke<void>('export_query_result_to_file', {
      name: this.uuid,
      sql,
      kind,
      file,
    });
  }

  setAutoCommit(commit: boolean): Promise<void> {
    return invoke<void>('query_set_autocommit', {
      name: this.uuid,
      queryId: this.queryId,
      commit,
    });
  }
  getAutoCommit(): Promise<boolean> {
    return invoke<boolean>('query_get_commit', {
      name: this.uuid,
      queryId: this.queryId,
    });
  }
  commit(): Promise<void> {
    return invoke<void>('query_commit', {
      name: this.uuid,
      queryId: this.queryId,
    });
  }
  rollback(): Promise<void> {
    return invoke<void>('query_rollback', {
      name: this.uuid,
      queryId: this.queryId,
    });
  }
  queryOP(sql: string): Promise<any[]> {
    let x = invoke<any[]>('query_sql', {
      name: this.uuid,
      queryId: this.queryId,
      sql,
    });
    return x;
  }
  executeOP(sql: string): Promise<any> {
    let x = invoke<any[]>('execute_sql', {
      name: this.uuid,
      queryId: this.queryId,
      sql,
    });
    return x;
  }
  loadQueryLog(): Promise<types.QueryLogEvent[]> {
    return invoke('query_logs', { name: this.uuid, queryId: this.queryId });
  }
  reconnect(): Promise<void> {
    return invoke('query_sql_re_connection', {
      name: this.uuid,
      queryId: this.queryId,
    });
  }
  isDangerSql(sql: string): Promise<boolean> {
    return invoke('is_dangerous_sql', { name: this.uuid, sql });
  }
  destroy() {
    console.log('destroy connection of:', this.queryId);
    invoke('query_sql_close_connection', {
      name: this.uuid,
      queryId: this.queryId,
    })
      .then(() => {})
      .catch((e) => {
        console.log('close connection failed,err', e);
      });
  }
}

function extractBaseName(file) {
  const fileName = file.match(/[^\\/]+$/)[0];
  return fileName;
}

function LoadStoreFileForQuery(props: {
  file;
  getQuery: () => string;
  handleChange: (file, content) => void;
  openOpt?: OpenDialogOptions;
  saveOpt?: SaveDialogOptions;
}) {
  const { file, getQuery, handleChange, openOpt, saveOpt } = props;
  const { showNotification } = useContext(NotificationContext);
  const loadFile = async () => {
    // ok to load
    try {
      let file = await open(openOpt);
      if (file) {
        const c = new TextDecoder().decode(await readFile(file));
        handleChange(file, c);
      }
    } catch (e) {
      showNotification('error', 'error to open file', e);
    }
  };
  const handleSave = async () => {
    let content = getQuery();
    if (!content) {
      showNotification('error', `can't save empty content`);
      return;
    }
    try {
      let ofile = file ? file : await save({ ...saveOpt });
      if (ofile) {
        await writeFile(ofile, new TextEncoder().encode(content), {
          create: true,
        });
        handleChange(ofile, content);
        showNotification('success', 'save file OK');
      }
    } catch (e) {
      showNotification('error', 'save file failed', e);
    }
  };

  const handleClose = async () => {
    try {
      let fileContent = new TextDecoder().decode(await readFile(file));
      if (fileContent != getQuery()) {
        const confirm = await secondConfirm({
          title: 'file not saved',
          content: 'are u sure to discard the content',
        });
        if (!confirm) {
          return;
        }
      }
      handleChange(null, '');
    } catch (e) {
      showNotification('error', 'close file failed', e);
    }
  };

  return (
    <>
      {file && (
        <Stack direction="row" style={{ alignItems: 'center' }}>
          <Tooltip title={file}>
            <Typography>{extractBaseName(file)}</Typography>
          </Tooltip>
          <Tooltip title="close file">
            <IconButton onClick={handleClose}>
              <CloseIcon></CloseIcon>
            </IconButton>
          </Tooltip>
        </Stack>
      )}
      <Tooltip title="attach file">
        <IconButton onClick={loadFile} color="primary">
          <AttachFileIcon />
        </IconButton>
      </Tooltip>
      <Tooltip title="save query to file">
        <IconButton onClick={handleSave} color="primary">
          <SaveIcon />
        </IconButton>
      </Tooltip>
    </>
  );
}
type MakeIQueryConn = () => Promise<IQueryConn>;
const keyNotInTable = 'qwertyuioplkjhgfdsazxcvbnmxwererewrewrew';

type EscDownEvent = { preventDefault: () => void };

export {
  modalStyle,
  DocsAndCount,
  EditorBar,
  SimpleResizeEditor,
  tabsHasId,
  msgTimeout,
  progressBarSize,
  progressBarSizeSmall,
  SettingContext,
  appStore,
  UserSettings,
  OpenedContext,
  MenuConnectionInfo,
  getNamedConfig,
  CreateDatabase,
  CreateSchema,
  dropDatabase,
  dropSchema,
  reloadOK,
  registeredLanguage,
  initMonacoLanague,
  QueryConn,
  AppStoreKey,
  LoadStoreFileForQuery,
  keyNotInTable,
  extractBaseName,
  makeAutoComplete,
};

export type { DefAndMetas, IQueryConn, MakeIQueryConn, EscDownEvent };
