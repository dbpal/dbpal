import {
  Button,
  ListItemIcon,
  ListItemText,
  MenuItem,
  Stack,
  SxProps,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';

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
  border: '1px solid #000',
  boxShadow: 24,
  margin: 0,
  p: 4,
};

import {
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
  extractConfigNameFromUuid,
  getViewportSize,
  secondConfirm,
  TabsSetContext,
} from './helper';
import { MetaValuesView } from './CommonComp';

const getNamedConfig = async (
  name: string
): Promise<types.DBConnectionConfig> => {
  let x = await (
    await appStore()
  ).get<types.DBConnectionConfig[]>(AppStoreKey.ConnectionConfig);
  if (!x) {
    return undefined;
  }
  return x.find((x) => x.name == name);
};

class DocsAndCount {
  constructor(public docs: BSON.Document[], public total: number) {}
}
enum MongoDataViewModel {
  List,
  Object,
}

function EditorBar(props: {
  left?: any;
  pos: EditorPositon;
  style?: CSSProperties | undefined;
}) {
  let { left, pos, style } = props;
  return (
    <Stack justifyContent="space-between" direction="row" style={style}>
      <div style={{ width: '40%' }}> {left}</div>
      <Stack width="40%" direction="row" spacing={1}>
        <span>Ln {pos ? pos.lineNumber : ''}</span>
        <span>Column {pos ? pos.column : ''}</span>
      </Stack>
    </Stack>
  );
}

function CreateDatabase({ uuid, handleClose }) {
  const { showNotification } = useContext(NotificationContext);
  const [database, setDatabase] = useState('');

  const handleCreate = () => {
    if (!database) {
      return;
    }

    invoke('create_database', { name: uuid, database })
      .then(() => {
        showNotification('success', 'Create OK');
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Create Failure', e);
      });
  };

  return (
    <Stack>
      <Stack>
        <TextField
          size="small"
          value={database}
          onChange={(e) => {
            setDatabase(e.target.value);
          }}
          error={database == ''}
          label="dadtabase name"
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

function CreateSchema(props: {
  uuid;
  handleClose;
  kind: DataSourceKind;
  schemas: string[];
}) {
  //@ts-ignore
  const { uuid, handleClose, kind, schemas } = props;
  const schemaSet = new Set(schemas);
  const { showNotification } = useContext(NotificationContext);
  const [schema, setSchema] = useState('');

  const handleCreate = () => {
    if (!schema) {
      showNotification('error', 'Schema is empty');
      return;
    }
    if (schemaSet.has(schema)) {
      showNotification('error', `'${schema}' already exists.`);
      return;
    }
    invoke('create_schema', { name: uuid, schema })
      .then(() => {
        showNotification('success', 'Create OK');
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Create Failure', e);
      });
  };

  if (kind == DataSourceKind.Redis) {
    // redis have special create
    const range = [];
    for (let i = 0; i < 16; i++) {
      range.push('db' + i);
    }

    return (
      <Stack>
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
    <Stack>
      <Stack>
        <TextField
          size="small"
          value={schema}
          onChange={(e) => {
            setSchema(e.target.value);
          }}
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

async function dropDatabase(props: {
  uuid: string;
  database;
  showNotification;
  okCallback?;
}) {
  const { uuid, database, showNotification, okCallback } = props;

  const confirm = await secondConfirm({
    title: `drop database ${database}`,
    confirmText: 'drop',
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
    confirmText: 'drop',
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

function countLines(str) {
  if (!str) return undefined; // 如果字符串为空，返回 0
  return str.split('\n').length; // 按换行符分割并计算长度
}

function editorInitHeight(
  str,
  lineHeigth,
  minHeight = 200,
  maxPrecent = 0.5
): number {
  let lines = countLines(str);
  if (!lines) {
    lines = 10;
  } else {
    lines += 2;
  }

  let height = lines * lineHeigth;
  let viewHeigth = getViewportSize().height;
  let maxHeigth = viewHeigth * maxPrecent;
  if (height > maxHeigth) {
    height = maxHeigth;
  }
  if (height < minHeight) {
    height = minHeight;
  }
  return height;
}

function SimpleResizeEditor(props: {
  language: string;
  value: string;
  onMount?: OnMount;
  options?: editor.IStandaloneEditorConstructionOptions;
  minHeigth?: number;
  barLeft?: ReactNode;
  style?: CSSProperties;
}) {
  let { language, value, onMount, options, minHeigth, barLeft, style } = props;
  const [pos, setPos] = useState<Position>(null);
  let height = editorInitHeight(
    value,
    useContext(SettingsContext).editorFontSize * 1.2
  );
  if (minHeigth && height < minHeigth) {
    height = minHeigth;
  }
  const handleDidMount = useCallback(
    (editor: editor.IStandaloneCodeEditor, monaco: Monaco) => {
      editor.onDidChangeCursorPosition(() => {
        setPos(editor.getPosition());
      });
      if (onMount) {
        onMount(editor, monaco);
      }
    },
    []
  );

  const { theme } = useContext(SettingsContext);
  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        resize: 'vertical',
        overflow: 'auto',
        ...style,
      }}
    >
      <div
        style={{
          flexGrow: 1,
          flexShrink: 1,
          minHeight: 0,
          height: height + 'px',
        }}
      >
        <Editor
          onMount={handleDidMount}
          language={language}
          value={value}
          theme={theme == 'dark' ? 'vs-dark' : 'light'}
          options={{
            fontSize: useContext(SettingsContext).editorFontSize,
            wordWrap: 'on',
            ...options,
          }}
        ></Editor>
      </div>
      <EditorBar left={barLeft} pos={pos}></EditorBar>
    </div>
  );
}

const msgTimeout = 5000;

function tabsHasId(tabs: TabContent[], id: string): number | null {
  for (let i = 0; i < tabs.length; i++) {
    let tab = tabs[i];
    if (tab.id == id) {
      return i;
    }
  }
  return null;
}

function extraParametersFromMetaValues(
  values: types.MetaValue[]
): string | null {
  for (const v of values) {
    if (v.name == 'parameters') {
      return v.value;
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
  const { tabs, setSelectIndex, setTabs } = useContext(TabsSetContext);

  const handleInfo = async () => {
    let id = uuid + '-info';
    let index = tabsHasId(tabs, id);
    if (typeof index == 'number') {
      setSelectIndex(index);
    } else {
      setTabs((e) => {
        setSelectIndex(tabs.length);
        return [
          ...e,
          new TabContent(
            id,
            uuid,
            <ConnConfigInfo uuid={uuid}></ConnConfigInfo>
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
      <ListItemText> info </ListItemText>
    </MenuItem>
  );
}

class UserSettings {
  theme: string = 'light';
  pageSize: number = 100;
  editorFontSize: number;

  constructor(theme?, pageSize?, editorFontSize?) {
    if (theme) {
      this.theme = theme;
    }
    if (pageSize) {
      this.pageSize = pageSize;
    }
    if (editorFontSize) {
      this.editorFontSize = editorFontSize;
    }
  }
}

const SettingsContext = createContext<UserSettings>(null);

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
        case 'mysql':
        case 'mariadb':
        case 'postgres':
        case 'redis':
        case 'mongo':
          configMetas = [
            { name: 'host', value: config.host } as types.MetaValue,
            { name: 'port', value: config.port + '' } as types.MetaValue,
            { name: 'user', value: config.username } as types.MetaValue,
          ];
          break;
        case 'sqlite':
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
      <div style={{ display: 'grid' }}>
        <MetaValuesView values={metas}></MetaValuesView>
      </div>
    </Stack>
  );
}
function reloadOK(showNotification) {
  showNotification('success', 'reload OK');
}
const registeredLanguage = new Set<string>();
const VisiableContext = createContext(true);

// uuid as language identifier.
function initMonacoLanague(uuid: string, kind: DataSourceKind) {
  if (registeredLanguage.has(uuid)) {
    return;
  }
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
    language: string,
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
    loader.init().then((monaco) => {
      monaco.languages.register({ id: language });
      monaco.languages.setMonarchTokensProvider(language, Ilanguage);
      if (format) {
        monaco.languages.registerDocumentFormattingEditProvider(language, {
          provideDocumentFormattingEdits(model, options, _token) {
            return format(model.getValue(), options).then((text) => [
              { range: model.getFullModelRange(), text },
            ]);
          },
        });
      }

      monaco.languages.registerCompletionItemProvider(language, {
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
    });
  }

  switch (kind) {
    case DataSourceKind.Mysql:
    case DataSourceKind.MariaDB:
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [mysqlKeywords, 'keyword'],
              [/[\$_a-z][\$_a-z0-9]*/i, 'identifier'],
              [/\"(?:\\.|[^\\\"])*\"/, 'string'],
              [/\'(?:\\.|[^\\\'])*\'/, 'string'],
              [/\`(?:\\.|[^\\\`])*\`/, 'string'],
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
              [/\"(?:\\.|[^\\\"])*\"/, 'string'],
              [/\'(?:\\.|[^\\\'])*\'/, 'string'],
              [/\`(?:\\.|[^\\\`])*\`/, 'string'],
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
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [sqliteKeywords, 'keyword'],
              [/[_a-z][_a-z0-9]*/i, 'identifier'],
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

    default:
      throw `impl this ${kind}`;
  }
  registeredLanguage.add(uuid);
}

async function makeQueryConnInfo(uuid: string) {
  const name = extractConfigNameFromUuid(uuid);
  const config = await getNamedConfig(name);
  if (!config) {
    return;
  }
  let content;
  switch (config.conn_type) {
    case 'mysql':
    case 'mariadb':
    case 'postgres':
    case 'sqlserver':
      content = (
        <>
          <strong>{config.conn_type}</strong>
          <span>
            {config.username}@{config.host}:{config.port}/
          </span>
        </>
      );
      break;
    case 'sqlite':
      content = (
        <>
          <strong>sqlite</strong>
          <span> {config.path}</span>
        </>
      );
      break;
    case 'redis':
      content = (
        <>
          <strong>{config.conn_type}</strong>
          <span>
            {config.username} {config.username && '@'}
            {config.host}:{config.port}
          </span>
        </>
      );
      break;
  }
  return (
    <Stack direction="row" spacing={1}>
      {content}
    </Stack>
  );
}

interface IQueryConn {
  queryOP: (text: string) => Promise<any[]>;
  loadQueryLog: () => Promise<types.QueryLogEvent[]>;
  reconnect: () => Promise<void>;
  isDangerSql: (sql: string) => Promise<boolean>;
  destroy: () => void;
  oracleQuerySetAutocommit: (boolean) => Promise<void>;
  oracleQueryAutocommit: () => Promise<boolean>;
  oracleQueryCommit: () => Promise<void>;
  oracleQueryRollback: () => Promise<void>;
  isOracle: boolean;
}

class QueryConn implements IQueryConn {
  queryId: string;
  uuid: string;
  isOracle: boolean;

  constructor(uuid, queryId: string, isOracle = false) {
    this.queryId = queryId;
    this.uuid = uuid;
    this.isOracle = isOracle;
  }

  oracleQuerySetAutocommit(commit: boolean): Promise<void> {
    return invoke<void>('oracle_query_set_autocommit', {
      name: this.uuid,
      queryId: this.queryId,
      commit,
    });
  }
  oracleQueryAutocommit(): Promise<boolean> {
    return invoke<boolean>('oracle_query_autocommit', {
      name: this.uuid,
      queryId: this.queryId,
    });
  }
  oracleQueryCommit(): Promise<void> {
    return invoke<void>('oracle_query_commit', {
      name: this.uuid,
      queryId: this.queryId,
    });
  }
  oracleQueryRollback(): Promise<void> {
    return invoke<void>('oracle_query_rollback', {
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

function LoadStoreQuery(props: {
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
    try {
      let ofile = file ? file : await save({ ...saveOpt });
      if (ofile) {
        await writeFile(ofile, new TextEncoder().encode(getQuery()), {
          create: true,
        });
        handleChange(ofile, getQuery());
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
        <>
          <Button onClick={handleClose}> close </Button>
          <Tooltip title={file}>
            <Typography> {extractBaseName(file)}</Typography>
          </Tooltip>
        </>
      )}
      <Button onClick={loadFile}>
        <AttachFileIcon></AttachFileIcon>
      </Button>

      <Button onClick={handleSave}>save</Button>
    </>
  );
}

type MakeIQueryConn = () => Promise<IQueryConn>;
const keyNotInTable = 'qwertyuioplkjhgfdsazxcvbnm';
export {
  modalStyle,
  DocsAndCount,
  MongoDataViewModel,
  EditorBar,
  SimpleResizeEditor,
  editorInitHeight,
  tabsHasId,
  msgTimeout,
  progressBarSize,
  progressBarSizeSmall,
  extraParametersFromMetaValues,
  SettingsContext,
  appStore,
  UserSettings,
  VisiableContext,
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
  makeQueryConnInfo,
  QueryConn,
  AppStoreKey,
  LoadStoreQuery,
  keyNotInTable,
};

export type { DefAndMetas, IQueryConn, MakeIQueryConn };
