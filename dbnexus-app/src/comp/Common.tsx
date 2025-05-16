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
  borderRadius: '5px',
  p: 1,
  maxHeight: '85vh',
  overflowY: 'auto',
};

import {
  cassandraKeywords,
  firebirdKeywords,
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

function countLines(str): number | undefined {
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
  value?: string;
  onMount?: OnMount;
  minHeigth?: number;
  barLeft?: ReactNode;
  style?: CSSProperties;
  wordWrap?: string;
  readOnly?: boolean;
}) {
  let {
    language,
    value,
    onMount,
    minHeigth,
    barLeft,
    style,
    wordWrap,
    readOnly,
  } = props;
  const [pos, setPos] = useState<Position>(null);
  const setting = useContext(SettingContext);
  let height = editorInitHeight(value, setting.editorFontSize * 1.2);
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
  editorFontSize: number;
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
    loader.init().then((monaco) => {
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
              [
                /\b(COLUMN|BETWEEN|INVOKER|RESOURCE|NAMED|SEMI|ONLY|STEP|WHERE|FUNCTIONS|FREEZE|MS|SET|VISIBLE|COLUMNS|NAME|FIELDS|MATERIALIZED|CASE|DOUBLE_SHA1_PASSWORD|KEY|SQL_TSI_NANOSECOND|ROLES|CLONE|DETACHED|ASCENDING|DATE|EXCHANGE|OUTFILE|MILLISECOND|SECURITY|EMPTY|NANOSECOND|COMMIT|DELAY|PERIODIC|HOURS|ORDER|REPLICATED|VALID|COMPRESSION|PASTE|SQL_TSI_MICROSECOND|REALM|PLAINTEXT_PASSWORD|STDOUT|SQL_TSI_SECOND|MERGES|ROLE|LOCAL|CHAR|MICROSECOND|STORAGE|SYSTEM|ADD|ID|PART|MI|OUTER|PROFILES|IN|DATABASES|GLOBAL|HOUR|INJECTIVE|INTO|FOLLOWING|KEYED|GROUP|NANOSECONDS|PROTOBUF|METHODS|RESTRICT|S|SALT|CLEANUP|AZURE|NULL|QQ|SUBPARTITIONS|QUARTERS|POPULATE|IGNORE|TRIGGER|PROCESSLIST|USING|STALENESS|TRUE|FALSE|MCS|REVOKE|AUTO_INCREMENT|CURRENT_USER|DOUBLE_SHA1_HASH|CODEC|PERMANENTLY|SQL_TSI_QUARTER|SSL_CERTIFICATE|INVISIBLE|CN|PROFILE|CURRENTUSER|DROP|FETCH|UPDATE|WATCH|FORMAT|CREATE|MONTHS|SQL_TSI_WEEK|ROW|UNTIL|SOURCE|EXTENDED|COLLATE|CROSS|IS|ALGORITHM|METRICS|ACTION|UUID|MINUTES|OVERRIDABLE|FORGET|FULLTEXT|READONLY|RECOMPRESS|DICTIONARY|FOR|EXCEPT|DAY|DAYS|TTL|RIGHT|FILTER|SAN|PROJECTION|CASCADE|M|NONE|PRIVILEGES|FUNCTION|DIV|DDL|SECONDS|START|CHANGED|THAN|SAMPLE|HIERARCHICAL|TYPE|ARRAY|PREWHERE|REGEXP|TYPEOF|YEARS|SQL_TSI_YEAR|BACKUP|MONTH|RESUME|MODIFY|TOP|N|LESS|SQL_TSI_DAY|PLAN|WORKLOAD|TIME|LAYOUT|PIPELINE|MAX|BIDIRECTIONAL|INDICES|COLLECTION|AFTER|CLUSTER|ENFORCED|Q|AUTHENTICATION|ACCESS|QUALIFY|MEMORY|BOTH|CLUSTERS|H|CHANGE|CACHE|HASH|NULLS|READ|FROM|SCHEME|STRICT|SS|WHEN|INFILE|SHA256_PASSWORD|JWT|MIN|UNSET|NOT|CLUSTER_HOST_IDS|AND|SQL_TSI_HOUR|S3|CONSTRAINT|ANY|CAST|TRAILING|RECURSIVE|WINDOW|SQL_TSI_MONTH|POLICY|DEFAULT|PRECEDING|PRECISION|INHERIT|ASC|APPLY|ELSE|BY|OBJECT|MATCH|HDFS|INTERVAL|MOVE|INDEX|DD|PREFIX|FORCE|UNDROP|YY|RESET|DATA|REFRESH|TAG|LIST|MICROSECONDS|SEQUENTIAL|PARTITION|EXPRESSION|PARALLEL|SIGNED|EXPLAIN|RANDOMIZED|THEN|PART_MOVE_TO_SHARD|CHECK|TABLES|OPTION|QUOTA|SUBPARTITION|SUSPEND|GROUPING|LAST|YEAR|CHANGEABLE_IN_READONLY|TRUNCATE|REFERENCES|KERBEROS|LIMIT|TIES|BASE_BACKUP|WEEKS|KILL|ROLLUP|DESCRIBE|END|SPATIAL|ALIAS|GRANTS|INSERT|ALTER|FINAL|IF|MOD|DATABASE|BAGEXPANSION|EPHEMERAL|REPLACE|RANDOMIZE|ATTACH|RANGE|LINEAR|RESTRICTIVE|ENGINES|SETS|INTERSECT|SYNC|TAGS|MUTATION|UNSIGNED|ASSUME|BEGIN|VALUES|WATERMARK|TRACKING|DISTINCT|RENAME|FILL|PRIMARY|WRITABLE|NS|SHARD|IP|LEADING|CLEAR|DEFINER|RESTORE|FAKE|TOTALS|USE|BCRYPT_PASSWORD|LIKE|RESPECT|TO|GRANTEES|BCRYPT_HASH|STATISTICS|ADMIN|DETACH|UNBOUNDED|PULL|TEMPORARY|ON|TRANSACTION|GRANT|SQL|USER|CHARACTER|HAVING|OVER|PARTITIONS|ROLLBACK|URL|COMMENT|DEDUPLICATE|GROUPS|SECOND|SIMPLE|IMPLICIT|CUBE|FULL|VIEW|IDENTIFIED|QUERY|EXTERNAL|NEXT|SQL_TSI_MILLISECOND|SQL_TSI_MINUTE|SSH_KEY|INNER|TREE|APPEND|LIGHTWEIGHT|NEW|VARYING|D|ROWS|WRITE|GRANULARITY|STRICTLY_ASCENDING|WITH_ITEMINDEX|EVENT|AST|FIRST|MINUTE|IS_OBJECT_ID|DEPENDS|ZKPATH|EXISTS|LEFT|QUARTER|SNAPSHOT|ANTI|ENGINE|MILLISECONDS|ILIKE|LEVEL|SYNTAX|WK|INTERPOLATE|REMOVE|HOST|SHOW|CONNECTIONS|NO|TEST|SETTINGS|AS|CONST|DICTIONARIES|SETTING|OPTIMIZE|ASOF|SHA256_HASH|SKIP|SERVER|UNIQUE|DESC|DISK|EVENTS|TABLE|ASYNC|ESTIMATE|EVERY|FOREIGN|LIMITS|LIFETIME|CURRENT|DELETE|MATERIALIZE|ALL|OR|OVERRIDE|TIMESTAMP|UNION|MASK|FILESYSTEM|FILE|WITH|KEYS|OFFSET|PERSISTENT|DESCENDING|CACHES|LARGE|PARTIAL|WEEK|LIVE|WW|YYYY|ALLOWED_LATENESS|PERMISSIVE|ENABLED|NO_PASSWORD|INDEXES|HTTP|KIND|VOLUME|SELECT|UNFREEZE|MM|JOIN|LDAP|HH|DELETED)\b/,
                'keyword',
              ],
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
    case DataSourceKind.LibSql:
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
    case DataSourceKind.DuckDB:
      initLanguageXXX(
        uuid,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [
                /\b(abort|absolute|access|action|add|admin|after|aggregate|all|also|alter|always|analyse|analyze|and|anti|any|array|as|asc|asof|assertion|assignment|asymmetric|at|attach|attribute|authorization|backward|before|begin|between|bigint|binary|bit|boolean|both|by|cache|call|called|cascade|cascaded|case|cast|catalog|centuries|century|chain|char|character|characteristics|check|checkpoint|class|close|cluster|coalesce|collate|collation|column|columns|comment|comments|commit|committed|compression|concurrently|configuration|conflict|connection|constraint|constraints|content|continue|conversion|copy|cost|create|cross|csv|cube|current|cursor|cycle|data|database|day|days|deallocate|dec|decade|decades|decimal|declare|default|defaults|deferrable|deferred|definer|delete|delimiter|delimiters|depends|desc|describe|detach|dictionary|disable|discard|distinct|do|document|domain|double|drop|each|else|enable|encoding|encrypted|end|enum|escape|event|except|exclude|excluding|exclusive|execute|exists|explain|export|export_state|extension|extensions|external|extract|false|family|fetch|filter|first|float|following|for|force|foreign|forward|freeze|from|full|function|functions|generated|glob|global|grant|granted|group|grouping|grouping_id|groups|handler|having|header|hold|hour|hours|identity|if|ignore|ilike|immediate|immutable|implicit|import|in|include|including|increment|index|indexes|inherit|inherits|initially|inline|inner|inout|input|insensitive|insert|install|instead|int|integer|intersect|interval|into|invoker|is|isnull|isolation|join|json|key|label|language|large|last|lateral|leading|leakproof|left|level|like|limit|listen|load|local|location|lock|locked|logged|macro|map|mapping|match|materialized|maxvalue|method|microsecond|microseconds|millennia|millennium|millisecond|milliseconds|minute|minutes|minvalue|mode|month|months|move|name|names|national|natural|nchar|new|next|no|none|not|nothing|notify|notnull|nowait|null|nullif|nulls|numeric|object|of|off|offset|oids|old|on|only|operator|option|options|or|order|ordinality|others|out|outer|over|overlaps|overlay|overriding|owned|owner|parallel|parser|partial|partition|passing|password|percent|persistent|pivot|pivot_longer|pivot_wider|placing|plans|policy|position|positional|pragma|preceding|precision|prepare|prepared|preserve|primary|prior|privileges|procedural|procedure|program|publication|qualify|quarter|quarters|quote|range|read|real|reassign|recheck|recursive|ref|references|referencing|refresh|reindex|relative|release|rename|repeatable|replace|replica|reset|respect|restart|restrict|returning|returns|revoke|right|role|rollback|rollup|row|rows|rule|sample|savepoint|schema|schemas|scope|scroll|search|second|seconds|secret|security|select|semi|sequence|sequences|serializable|server|session|set|setof|sets|share|show|similar|simple|skip|smallint|snapshot|some|sql|stable|standalone|start|statement|statistics|stdin|stdout|storage|stored|strict|strip|struct|subscription|substring|summarize|symmetric|sysid|system|table|tables|tablesample|tablespace|temp|template|temporary|text|then|ties|time|timestamp|to|trailing|transaction|transform|treat|trigger|trim|true|truncate|trusted|try_cast|type|types|unbounded|uncommitted|unencrypted|union|unique|unknown|unlisten|unlogged|unpivot|until|update|use|user|using|vacuum|valid|validate|validator|value|values|varchar|variable|variadic|varying|verbose|version|view|views|virtual|volatile|week|weeks|when|where|whitespace|window|with|within|without|work|wrapper|write|xml|xmlattributes|xmlconcat|xmlelement|xmlexists|xmlforest|xmlnamespaces|xmlparse|xmlpi|xmlroot|xmlserialize|xmltable|year|years|yes|zone)\b/,
                'keyword',
              ],
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
    case DataSourceKind.Cockroach:
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

async function makeQueryConnInfo(uuid: string) {
  const name = extractConfigNameFromUuid(uuid);
  const config = await getNamedConfig(name);
  if (!config) {
    return;
  }
  let content;
  let s = {
    fontSize: '0.9rem',
    color: 'gray',
  } as CSSProperties;

  switch (config.conn_type) {
    case DataSourceKind.Mysql:
    case DataSourceKind.MariaDB:
    case DataSourceKind.Postgres:
    case DataSourceKind.MsSql:
    case DataSourceKind.Cassandra:
    case DataSourceKind.ClickHouse:
    case DataSourceKind.Cockroach:
      content = (
        <div style={s}>
          <strong>{config.conn_type}</strong>&nbsp;
          <span>
            {config.username}@{config.host}:{config.port}/
          </span>
        </div>
      );
      break;
    case DataSourceKind.Sqlite:
      content = (
        <div style={s}>
          <strong>sqlite</strong>&nbsp;
          <span> {config.path}</span>
        </div>
      );
      break;
    case DataSourceKind.Redis:
      content = (
        <div style={s}>
          <strong>{config.conn_type}</strong>&nbsp;
          <span>
            {config.username} {config.username && '@'}
            {config.host}:{config.port}
          </span>
        </div>
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
const keyNotInTable = 'qwertyuioplkjhgfdsazxcvbnm';

type EscDownEvent = { preventDefault: () => void };

export {
  modalStyle,
  DocsAndCount,
  EditorBar,
  SimpleResizeEditor,
  editorInitHeight,
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
  makeQueryConnInfo,
  QueryConn,
  AppStoreKey,
  LoadStoreFileForQuery,
  keyNotInTable,
  extractBaseName,
  makeAutoComplete,
};

export type { DefAndMetas, IQueryConn, MakeIQueryConn, EscDownEvent };
