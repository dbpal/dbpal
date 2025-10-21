'use strict';
import {
  createContext,
  CSSProperties,
  RefObject,
  useContext,
  useRef,
  useState,
} from 'react';
import ClearIcon from '@mui/icons-material/Clear';
import {
  TextField,
  Button,
  Switch,
  FormControlLabel,
  Typography,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Stack,
  Divider,
  RadioGroup,
  Radio,
  Checkbox,
  Chip,
  Tooltip,
  InputAdornment,
  IconButton,
  useTheme,
} from '@mui/material';
import AttachFileIcon from '@mui/icons-material/AttachFile';
import VisibilityIcon from '@mui/icons-material/Visibility';
import { NotificationContext } from './Notification';
import { types } from './ApiStruct';
import { message, open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { AppStoreKey, extractBaseName } from './Common';
import { appStore, DataSourceKind, errToString } from './helper';
import { openUrl } from '@tauri-apps/plugin-opener';
import { platform } from '@tauri-apps/plugin-os';
import { UrlDocument } from './CommonComp2';

async function selectOneFile() {
  const selected = await open({
    multiple: false,
  });
  return selected;
}

function HostAndPort(props: { host; port; setHost; setPort; onBlur? }) {
  const { host, port, setHost, setPort, onBlur } = props;
  return (
    <Stack direction="row">
      <TextField
        fullWidth
        label="Host"
        value={host}
        onBlur={onBlur}
        sx={{ mr: 1 }}
        onChange={(e) => setHost(e.target.value)}
        slotProps={{
          input: {
            endAdornment: host && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setHost('');
                  }}
                  edge="end"
                >
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
        }}
        required
      />
      <TextField
        fullWidth
        label="port"
        onBlur={onBlur}
        error={isNaN(parseInt(port))}
        value={port}
        onChange={(e) => setPort(e.target.value)}
      />
    </Stack>
  );
}

function SelectPath(props: {
  path;
  setPath;
  label: string;
  required: boolean;
}) {
  const { path, setPath, label, required } = props;
  return (
    <Stack direction="row" sx={{ alignItems: 'center' }}>
      <TextField
        value={path}
        onChange={(e) => {
          setPath(e.target.value);
        }}
        style={{ flex: 1 }}
        label={label}
        required={required}
        slotProps={{
          input: {
            endAdornment: path && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setPath('');
                  }}
                  edge="end"
                >
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
        }}
      ></TextField>
      <IconButton
        onClick={() => {
          selectOneFile().then((e) => {
            setPath(e);
          });
        }}
        color="primary"
      >
        <AttachFileIcon></AttachFileIcon>
      </IconButton>
    </Stack>
  );
}

function UserAndPassword(props: {
  user;
  password;
  setUser;
  setPassword;
  savePassword;
  setSavePassword;
}) {
  const {
    user,
    password,
    setUser,
    setPassword,
    savePassword,
    setSavePassword,
  } = props;
  const [passwordVisible, setPasswordVisible] = useState(false);
  return (
    <Stack direction="row">
      <TextField
        fullWidth
        label="User"
        value={user}
        sx={{ mr: 1 }}
        onChange={(e) => setUser(e.target.value)}
        slotProps={{
          input: {
            endAdornment: user && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setUser('');
                  }}
                  edge="end"
                >
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
        }}
      />
      <TextField
        fullWidth
        label="Password"
        type={passwordVisible ? 'text' : 'password'}
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        slotProps={{
          input: {
            endAdornment: !passwordVisible &&
              password != '' &&
              platform() != 'windows' && ( // window 已经有了这个功能了
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setPasswordVisible(true);
                    }}
                    edge="end"
                  >
                    <VisibilityIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
          },
        }}
      />
      <Stack
        style={{
          width: '31rem',
          justifyContent: 'center',
          alignItems: 'center',
        }}
        direction="row"
      >
        <Checkbox
          checked={savePassword}
          onChange={(e) => {
            setSavePassword(e.target.checked);
          }}
          name="save password"
        />
        save password
      </Stack>
    </Stack>
  );
}

function Https(props: { https; setHttps }) {
  const { https, setHttps } = props;
  return (
    <FormControlLabel
      control={
        <Switch checked={https} onChange={(e) => setHttps(e.target.checked)} />
      }
      label="HTTPS"
    />
  );
}

function SqlServerCert(props: { data: SqlServerCertData; setData }) {
  const { data, setData } = props;
  return (
    <Stack direction="row" style={{ alignItems: 'center' }}>
      <FormControlLabel
        control={
          <Switch
            checked={data.trust_cert}
            onChange={(e) =>
              setData({
                ...data,
                trust_cert: e.target.checked,
              } as SqlServerCertData)
            }
          />
        }
        label="Trust Cert"
      />
      <Chip
        label="?"
        onClick={() => {
          openUrl(
            'https://docs.rs/tiberius/0.12.3/tiberius/struct.Config.html#method.trust_cert'
          );
        }}
      ></Chip>
    </Stack>
  );
}

interface SqlServerCertData {
  trust_cert: boolean;
  cert_ca: string | null;
}

function SslFiles(props: {
  files: types.SSLFiles;
  setFiles;
  enabled: boolean;
}) {
  const { files, setFiles, enabled } = props;

  return (
    enabled && (
      <>
        <Stack direction="row" spacing={1} style={{ flexWrap: 'wrap' }}>
          <Tooltip
            title={files.ssl_ca}
            onClick={async () => {
              const file = await selectOneFile();
              if (file) {
                setFiles({ ...files, ssl_ca: file });
              }
            }}
          >
            <Chip
              label={
                'CA Cert:' + (files.ssl_ca ? extractBaseName(files.ssl_ca) : '')
              }
              variant="outlined"
            ></Chip>
          </Tooltip>
          <Tooltip title={files.ssl_cert}>
            <Chip
              variant="outlined"
              onClick={async () => {
                const file = await selectOneFile();
                if (file) {
                  setFiles({ ...files, ssl_cert: file });
                }
              }}
              label={
                'Certificate:' +
                (files.ssl_cert ? extractBaseName(files.ssl_cert) : '')
              }
            ></Chip>
          </Tooltip>
          <Tooltip title={files.ssl_key}>
            <Chip
              variant="outlined"
              onClick={async () => {
                const file = await selectOneFile();
                if (file) {
                  setFiles({ ...files, ssl_key: file });
                }
              }}
              label={
                'Key File:' +
                (files.ssl_key ? extractBaseName(files.ssl_key) : '')
              }
            ></Chip>
          </Tooltip>
        </Stack>
      </>
    )
  );
}

function PGSSL(props: { ssl: types.PgSsl; setSsl: any }) {
  const { ssl, setSsl } = props;
  const modes = [
    'disable',
    'allow',
    'prefer',
    'require',
    'verifyca',
    'verifyfull',
  ] as types.PgSslMode[];

  return (
    <Stack
      direction="row"
      spacing={1}
      style={{ alignItems: 'center', flexWrap: 'wrap' }}
    >
      <FormControl style={{ flex: 1 }}>
        <InputLabel> ssl mode </InputLabel>
        <Select
          value={ssl.ssl_mode}
          label="candidates"
          onChange={(e) =>
            setSsl({ ...ssl, ssl_mode: e.target.value } as types.PgSsl)
          }
        >
          {modes.map((e) => (
            <MenuItem value={e} key={e}>
              {e}
            </MenuItem>
          ))}
        </Select>
      </FormControl>
      <UrlDocument url="https://docs.rs/sqlx/latest/sqlx/postgres/enum.PgSslMode.html"></UrlDocument>

      <SslFiles
        enabled={ssl.ssl_mode != 'disable'}
        files={ssl}
        setFiles={setSsl}
      />
    </Stack>
  );
}
function MysqlSSl(props: { ssl: types.MysqlSsl; setSsl: any }) {
  const { ssl, setSsl } = props;
  const modes = [
    'disabled',
    'preferred',
    'required',
    'verifyca',
    'verifyidentity',
  ] as types.MySqlSslMode[];
  return (
    <Stack
      direction="row"
      spacing={1}
      style={{ alignItems: 'center', flexWrap: 'wrap' }}
    >
      <FormControl style={{ flex: 1 }}>
        <InputLabel> ssl mode </InputLabel>
        <Select
          value={ssl.ssl_mode}
          label="candidates"
          onChange={(e) =>
            setSsl({ ...ssl, ssl_mode: e.target.value } as types.MysqlSsl)
          }
        >
          {modes.map((e) => (
            <MenuItem value={e} key={e}>
              {e}
            </MenuItem>
          ))}
        </Select>
      </FormControl>
      <UrlDocument url="https://docs.rs/sqlx/latest/sqlx/mysql/enum.MySqlSslMode.html"></UrlDocument>

      <SslFiles
        enabled={ssl.ssl_mode != 'disabled'}
        files={ssl}
        setFiles={setSsl}
      />
    </Stack>
  );
}

function DbSchemaCadidates(props: {
  name;
  setName;
  label: string;
  makeConfig: MakeConfig;
}) {
  const { name, setName, label, makeConfig } = props;
  const [candidates, setCadidates] = useState([]);
  const { showNotification } = useContext(NotificationContext);
  const handleReload = () => {
    const config = makeConfig();
    if (!config.name) {
      config.name = 'test_conn';
    }
    invoke<string[]>('config_list_database_or_schema', { conn: config })
      .then((e) => {
        showNotification('success', 'fetch candidates success');
        setCadidates(e);
      })
      .catch((e) => {
        showNotification('error', `list ${label} failed`, e);
      });
  };
  return (
    <Stack direction="row" spacing={1}>
      {candidates.length > 0 && (
        <FormControl style={{ flex: 1 }}>
          <InputLabel> candidates </InputLabel>
          <Select
            value={name}
            label="candidates"
            onChange={(e) => setName(e.target.value)}
          >
            {candidates.map((e) => (
              <MenuItem value={e} key={e}>
                {e}
              </MenuItem>
            ))}
          </Select>
        </FormControl>
      )}

      <TextField
        label={label}
        style={{ flex: 1 }}
        value={name}
        onChange={(e) => setName(e.target.value)}
        slotProps={{
          input: {
            endAdornment: name && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setName('');
                  }}
                  edge="end"
                >
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
        }}
      />
      <Button onClick={handleReload}> load candidates </Button>
    </Stack>
  );
}

type MakeConfig = () => types.DBConnectionConfig;

function Mysql(props: { config?: types.MysqlDBConfig; kind: DataSourceKind }) {
  const { config, kind } = props;
  const { connectionName, connTimeOut, readOnly } =
    useContext(AddConnectionContext);
  let defaultPort = 3306;
  if (kind == DataSourceKind.TiDB) {
    defaultPort = 4000;
  }
  const [host, setHost] = useState(config?.host ? config.host : '');
  const [port, setPort] = useState((config?.port || defaultPort) + '');
  const [user, setUser] = useState(config?.username);
  const [password, setPassword] = useState(
    config?.password ? config.password : ''
  );
  const [database, setDatabase] = useState(config?.db);
  const [ssl, setSsl] = useState<types.MysqlSsl>({
    ssl_mode: config?.ssl_mode || 'preferred',
    ssl_ca: config?.ssl_ca,
    ssl_cert: config?.ssl_cert,
    ssl_key: config?.ssl_key,
  });
  const [savePassword, setSavePassword] = useState(
    config ? config.save_password : true
  );

  function makeConfig(): types.DBConnectionConfig {
    let x: types.MysqlDBConfig = {
      host,
      port: parseInt(port),
      username: user,
      password: password ? password : null,
      save_password: savePassword,
      db: database ? database : null,
      read_only: readOnly,
      conn_timeout: connTimeOut,
      ...ssl,
    };
    switch (kind) {
      case DataSourceKind.Mysql:
      case DataSourceKind.MariaDB:
      case DataSourceKind.TiDB:
        return {
          conn_type: kind,
          name: connectionName,
          ...x,
        } as types.DBConnectionConfig;

      default:
        message('pass wrong kind of parameter, kind:' + kind, {
          kind: 'error',
        });
    }
  }
  globalMakeConfig = makeConfig;

  return (
    <Stack spacing={1}>
      <HostAndPort
        host={host}
        port={port}
        setHost={setHost}
        setPort={setPort}
      ></HostAndPort>
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      ></UserAndPassword>
      <DbSchemaCadidates
        name={database}
        label="database"
        setName={setDatabase}
        makeConfig={makeConfig}
      ></DbSchemaCadidates>
      <MysqlSSl ssl={ssl} setSsl={setSsl} />
    </Stack>
  );
}

function Oracle(props: { config?: types.OracleDBConfig }) {
  const { config } = props;
  const [host, setHost] = useState(config?.host ? config.host : '');
  const [port, setPort] = useState((config?.port || 1521) + '');
  const [user, setUser] = useState(config?.username ? config.username : '');
  const [password, setPassword] = useState(
    config?.password ? config.password : ''
  );
  const [database, setDatabase] = useState(config?.db);
  const [savePassword, setSavePassword] = useState(
    config ? config.save_password : true
  );
  const [schema, setSchema] = useState(config?.schema);
  const { connTimeOut, connectionName, readOnly } =
    useContext(AddConnectionContext);

  function makeConfig(): types.DBConnectionConfig {
    return {
      conn_type: 'oracle',
      name: connectionName,
      host,
      port: parseInt(port),
      username: user,
      password: password ? password : null,
      save_password: savePassword,
      db: database,
      schema: schema ? schema : null,
      conn_timeout: connTimeOut,
      read_only: readOnly,
    };
  }
  globalMakeConfig = makeConfig;
  return (
    <Stack spacing={1}>
      <Typography
        color="info"
        style={{ cursor: 'pointer' }}
        onClick={() => {
          openUrl(
            'https://odpi-c.readthedocs.io/en/latest/user_guide/installation.html'
          );
        }}
      >
        oracle need extra driver
      </Typography>
      <HostAndPort
        host={host}
        port={port}
        setHost={setHost}
        setPort={setPort}
      ></HostAndPort>
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      ></UserAndPassword>
      <TextField
        value={database}
        label="database"
        onChange={(e) => {
          setDatabase(e.target.value);
        }}
      ></TextField>
      {database && (
        <DbSchemaCadidates
          name={schema}
          setName={setSchema}
          makeConfig={makeConfig}
          label="schema"
        ></DbSchemaCadidates>
      )}
    </Stack>
  );
}

function ClickHouse(props: { config?: types.ClickHouseConfig }) {
  const { config } = props;
  const [host, setHost] = useState(config?.host ? config.host : '');
  const [port, setPort] = useState((config?.port || 3306) + '');
  const [user, setUser] = useState(config?.username ? config.username : '');
  const [password, setPassword] = useState(
    config?.password ? config.password : ''
  );
  const [database, setDatabase] = useState(config?.db);
  const [savePassword, setSavePassword] = useState(
    config ? config.save_password : true
  );
  const [https, setHttps] = useState(config?.https);
  const { connTimeOut, connectionName, readOnly } =
    useContext(AddConnectionContext);
  function makeConfig(): types.DBConnectionConfig {
    return {
      conn_type: 'clickhouse',
      name: connectionName,
      host,
      port: parseInt(port),
      username: user,
      password: password ? password : null,
      save_password: savePassword,
      db: database,
      https: https,
      read_only: readOnly,
      conn_timeout: connTimeOut,
    };
  }
  globalMakeConfig = makeConfig;

  return (
    <Stack spacing={1}>
      <HostAndPort
        host={host}
        port={port}
        setHost={setHost}
        setPort={setPort}
      ></HostAndPort>
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      ></UserAndPassword>
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        makeConfig={makeConfig}
        label="database"
      ></DbSchemaCadidates>
      <Https https={https} setHttps={setHttps}></Https>
    </Stack>
  );
}

function PostGres(props: { config?: types.PGDBConfig; kind: DataSourceKind }) {
  const { config, kind } = props;
  const [host, setHost] = useState(config?.host ? config.host : '');
  const [port, setPort] = useState<string>((config?.port || 5432) + '');

  const [user, setUser] = useState(config?.username ? config.username : '');
  const [password, setPassword] = useState(
    config?.password ? config.password : ''
  );
  const [database, setDatabase] = useState(config?.db);
  const [ssl, setSSl] = useState<types.PgSsl>({
    ssl_mode: config?.ssl_mode || 'prefer',
    ssl_ca: config?.ssl_ca,
    ssl_cert: config?.ssl_cert,
    ssl_key: config?.ssl_key,
  });
  const [schema, setSchema] = useState(config?.schema);
  const { connTimeOut, connectionName, readOnly } =
    useContext(AddConnectionContext);
  const [savePassword, setSavePassword] = useState(
    config ? config.save_password : true
  );

  function makeConfig(): types.DBConnectionConfig {
    let x: types.PGDBConfig = {
      host,
      port: parseInt(port),
      username: user,
      password: password ? password : null,
      save_password: savePassword,
      db: database ? database : null,
      read_only: readOnly,
      conn_timeout: connTimeOut,
      schema: schema ? schema : null,
      ...ssl,
    };
    switch (kind) {
      case DataSourceKind.Mysql:
      case DataSourceKind.MariaDB:
      case DataSourceKind.TiDB:
        return {
          conn_type: kind,
          name: connectionName,
          ...x,
        } as types.DBConnectionConfig;

      default:
        message('pass wrong kind of parameter, kind:' + kind, {
          kind: 'error',
        });
    }
  }
  globalMakeConfig = makeConfig;

  return (
    <Stack spacing={1}>
      <HostAndPort
        host={host}
        port={port}
        setHost={setHost}
        setPort={setPort}
      ></HostAndPort>
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      ></UserAndPassword>
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        makeConfig={makeConfig}
        label="database"
      ></DbSchemaCadidates>
      {database && (
        <DbSchemaCadidates
          name={schema}
          setName={setSchema}
          label="schema"
          makeConfig={makeConfig}
        ></DbSchemaCadidates>
      )}
      <PGSSL ssl={ssl} setSsl={setSSl} />
    </Stack>
  );
}

function Sqlite(props: { config?: types.SqliteDBConfig }) {
  const { config } = props;
  const [path, setPath] = useState(config?.path);
  const { connTimeOut, connectionName, readOnly } =
    useContext(AddConnectionContext);

  function makeConfig(): types.DBConnectionConfig {
    return {
      conn_type: 'sqlite',
      name: connectionName,
      path,
      conn_timeout: connTimeOut,
      read_only: readOnly,
    };
  }
  globalMakeConfig = makeConfig;

  return (
    <Stack spacing={1}>
      <SelectPath
        path={path}
        setPath={setPath}
        required
        label="sqlite database file"
      ></SelectPath>
    </Stack>
  );
}

const AddConnectionContext = createContext<{
  connectionName: string;
  readOnly: boolean;
  connTimeOut: number | null;
  connectionNameDom: RefObject<HTMLInputElement>;
  handleClose: () => void;
}>(null);

// global var make it simple the pass the parameter
// add connection run in separete window
let globalMakeConfig: MakeConfig = null;

function TestConnAndSave(props: {
  connectionNameDom;
  handleClose: () => void;
}) {
  const { connectionNameDom, handleClose } = props;
  const [testLoading, setTestLoading] = useState(false);
  const [testStatus, setTestStatus] = useState<string | true | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const handleTestConnection = async () => {
    const config = globalMakeConfig();
    if (!config.name) {
      config.name = 'test_connection';
    }
    if (config) {
      console.log('test config:', config);
      setTestLoading(true);
      invoke('test_connection', { config })
        .then(() => {
          setTestStatus(true);
        })
        .catch((e) => {
          setTestStatus(errToString(e));
          showNotification('error', 'test connection failed', e);
        })
        .finally(() => {
          setTestLoading(false);
          setTimeout(() => {
            setTestStatus(null);
          }, 5000);
        });
    }
  };

  const handleSaveConnection = async () => {
    const config = globalMakeConfig();
    if (!config) {
      return;
    }
    if (!config.name) {
      connectionNameDom.current.focus();
      showNotification('error', 'Connection Name is empty.');
      return;
    }
    const password = config['password'];
    const savePassword = config['save_password'];
    let connections = await appStore.get<types.DBConnectionConfig[]>(
      AppStoreKey.ConnectionConfig
    );
    if (!connections) {
      connections = [];
    }
    if (config.name.includes('/')) {
      showNotification('error', `connection name can't  contains '/'`);
      connectionNameDom.current.focus();
      return;
    }
    if (connections.find((e) => e.name == config.name)) {
      showNotification('error', 'name already been taken');
      connectionNameDom.current.focus();
      return;
    }
    if (password && !savePassword) {
      try {
        await invoke('save_entered_password', {
          name: config.name,
          pass: password,
          conn: config,
        });
      } catch (e) {
        showNotification('error', 'save entered password failed', e);
        return;
      }
    }
    if (password && savePassword) {
      try {
        await invoke('save_password', {
          name: config.name,
          pass: password,
        });
      } catch (e) {
        showNotification('error', 'save password failed', e);
        return;
      }
    }
    if (password) {
      config['password'] = '';
    }
    connections = [config, ...connections];
    appStore
      .set(AppStoreKey.ConnectionConfig, connections)
      .then((_) => {
        showNotification('success', 'Add Connection OK');
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Add Connection failure', e);
      });
  };
  return (
    <Stack spacing={1}>
      <Divider></Divider>
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <Button
          color="primary"
          onClick={() => {
            handleSaveConnection();
          }}
        >
          save
        </Button>
        <Button
          variant="outlined"
          sx={{ mr: 1 }}
          onClick={handleTestConnection}
          loading={testLoading}
        >
          test
        </Button>
        {testStatus === true && (
          <Typography color="success">Test OK</Typography>
        )}
        {typeof testStatus == 'string' && (
          <Typography color="error"> {testStatus} </Typography>
        )}
      </Stack>
    </Stack>
  );
}

function FireBird(props: { config?: types.FireBirdConfig }) {
  const { config } = props;
  const [host, setHost] = useState(config?.host ? config.host : '');
  const [port, setPort] = useState((config?.port || 3050) + '');
  const [user, setUser] = useState(config?.username ? config.username : '');
  const [password, setPassword] = useState(
    config?.password ? config.password : ''
  );
  const [savePassword, setSavePassword] = useState(
    config ? config.save_password : true
  );
  const { connTimeOut, connectionName, readOnly } =
    useContext(AddConnectionContext);
  const [database, setDatabase] = useState('');
  function makeConfig(): types.DBConnectionConfig {
    return {
      conn_type: 'firebird',
      name: connectionName,
      host,
      port: parseInt(port),
      username: user,
      password,
      save_password: savePassword,
      db: database,
      conn_timeout: connTimeOut,
      read_only: readOnly,
    };
  }
  globalMakeConfig = makeConfig;

  return (
    <Stack spacing={1}>
      <HostAndPort
        host={host}
        port={port}
        setHost={setHost}
        setPort={setPort}
      ></HostAndPort>
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      ></UserAndPassword>
      <TextField
        label="database"
        value={database}
        onChange={(e) => {
          setDatabase(e.target.value);
        }}
      ></TextField>
    </Stack>
  );
}

function Redis(props: { config?: types.RedisDBConfig }) {
  const { config } = props;
  const [host, setHost] = useState(config?.host ? config.host : '');
  const [port, setPort] = useState((config?.port || 6379) + '');
  const [user, setUser] = useState(config?.username ? config.username : '');
  const [password, setPassword] = useState(
    config?.password ? config.password : '' + ''
  );
  const [savePassword, setSavePassword] = useState(
    config ? config.save_password : true
  );
  const [database, setDatabase] = useState('');
  const { connectionName, connTimeOut, readOnly } =
    useContext(AddConnectionContext);
  const [enableSSL, setEnableSSL] = useState(!!config?.enable_ssl);
  function makeConfig(): types.DBConnectionConfig {
    return {
      name: connectionName,
      conn_type: 'redis',
      host,
      port: parseInt(port),
      username: user,
      password: password ? password : null,
      read_only: readOnly,
      conn_timeout: connTimeOut,
      db: database ? database : null,
      save_password: savePassword,
      enable_ssl: enableSSL,
    };
  }

  globalMakeConfig = makeConfig;

  return (
    <Stack spacing={1}>
      <HostAndPort
        host={host}
        port={port}
        setHost={setHost}
        setPort={setPort}
      ></HostAndPort>
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      ></UserAndPassword>
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        label="database"
        makeConfig={makeConfig}
      ></DbSchemaCadidates>
      <FormControlLabel
        control={
          <Switch
            checked={enableSSL}
            onChange={(e) => setEnableSSL(e.target.checked)}
          />
        }
        label="enable ssl"
      />
    </Stack>
  );
}

function MongoDb(props: { config?: types.MongoDBConfig }) {
  const { config } = props;
  const [host, setHost] = useState(config?.host ? config.host : '');
  const [port, setPort] = useState((config?.port || 27017) + '');
  const [user, setUser] = useState(config?.username ? config.username : '');
  const [password, setPassword] = useState(
    config?.password ? config.password : ''
  );
  const [savePassword, setSavePassword] = useState(
    config ? config.save_password : true
  );
  const [database, setDatabase] = useState('');

  const { connTimeOut, connectionName, readOnly } =
    useContext(AddConnectionContext);
  function makeConfig(): types.DBConnectionConfig {
    return {
      conn_type: 'mongo',
      name: connectionName,
      read_only: readOnly,
      host,
      username: user ? user : null,
      password: password ? password : null,
      conn_timeout: connTimeOut,
    };
  }
  globalMakeConfig = makeConfig;

  return (
    <Stack spacing={1}>
      <HostAndPort
        host={host}
        port={port}
        setHost={setHost}
        setPort={setPort}
      ></HostAndPort>
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      ></UserAndPassword>
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        label="database"
        makeConfig={makeConfig}
      ></DbSchemaCadidates>
    </Stack>
  );
}

function Cassandra(props: { config?: types.CassandraDBConfig }) {
  const { config } = props;
  const [host, setHost] = useState(config?.host ? config.host : '');
  const [port, setPort] = useState((config?.port || 9042) + '');
  const [user, setUser] = useState(config?.username ? config.username : '');
  const [password, setPassword] = useState(
    config?.password ? config.password : ''
  );
  const [savePassword, setSavePassword] = useState(
    config ? config.save_password : true
  );
  const [database, setDatabase] = useState('');
  const { connTimeOut, connectionName, readOnly } =
    useContext(AddConnectionContext);
  function makeConfig(): types.DBConnectionConfig {
    return {
      conn_type: 'cassandra',
      name: connectionName,
      host,
      username: user ? user : '',
      password: password ? password : null,
      db: database ? database : null,
      save_password: savePassword,
      read_only: readOnly,
      conn_timeout: connTimeOut,
      port: parseInt(port),
    };
  }

  globalMakeConfig = makeConfig;

  return (
    <Stack spacing={1}>
      <HostAndPort
        host={host}
        port={port}
        setHost={setHost}
        setPort={setPort}
      ></HostAndPort>
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      ></UserAndPassword>
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        label="database"
        makeConfig={makeConfig}
      ></DbSchemaCadidates>
    </Stack>
  );
}

function SqlServer(props: { config?: types.SqlServerDBConfig }) {
  const { config } = props;
  const [host, setHost] = useState(config?.host ? config.host : '');
  const [port, setPort] = useState((config?.port || 1433) + '');
  const [user, setUser] = useState(config?.username ? config.username : '');
  const [password, setPassword] = useState(
    config?.password ? config.password : ''
  );
  const [savePassword, setSavePassword] = useState(
    config ? config.save_password : true
  );

  const [database, setDatabase] = useState('');
  const [sqlserverCert, setSqlServerCert] = useState<SqlServerCertData>({
    trust_cert: config ? config.trust_cert : false,
    cert_ca: config ? config.cert_ca : '',
  });
  const { connTimeOut, connectionName, readOnly } =
    useContext(AddConnectionContext);

  function makeConfig(): types.DBConnectionConfig {
    return {
      conn_type: 'sqlserver',
      read_only: readOnly,
      conn_timeout: connTimeOut,
      username: user,
      password: password,
      name: connectionName,
      host,
      db: database ? database : null,
      port: parseInt(port),
      save_password: savePassword,
      trust_cert: sqlserverCert.trust_cert,
      cert_ca: sqlserverCert.cert_ca,
      schema: database ? 'dbo' : null,
    };
  }
  globalMakeConfig = makeConfig;

  return (
    <Stack spacing={1}>
      <HostAndPort
        host={host}
        port={port}
        setHost={setHost}
        setPort={setPort}
      ></HostAndPort>
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      ></UserAndPassword>
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        label="database"
        makeConfig={makeConfig}
      ></DbSchemaCadidates>
      <SqlServerCert
        data={sqlserverCert}
        setData={setSqlServerCert}
      ></SqlServerCert>
    </Stack>
  );
}

function AddConnection(props: {
  handleClose: () => void;
  config?: types.DBConnectionConfig;
}) {
  let { handleClose, config } = props;
  const [connectionType, setConnectionType] = useState(
    config?.conn_type || 'mysql'
  );
  const [connectionName, setConnectionName] = useState(config?.name);
  const [connTimeout, setConnTimeout] = useState<string>(
    config ? config.conn_timeout.toString() : ''
  );
  const connectionNameDom = useRef<HTMLInputElement>(null);
  const [readOnly, setReadOnly] = useState(config?.read_only);
  const theme = useTheme();
  const tabCss: CSSProperties = {
    padding: '1rem',

    borderRadius: '0.7rem',
  };
  return (
    <Stack spacing={1}>
      <FormControl fullWidth>
        <InputLabel>Connection Type</InputLabel>
        <Select
          value={connectionType}
          label="Connection Type"
          onChange={(e) => setConnectionType(e.target.value as DataSourceKind)}
          disabled={!!config}
        >
          <MenuItem value={DataSourceKind.Mysql}>MySQL</MenuItem>
          <MenuItem value={DataSourceKind.MariaDB}>MariaDB</MenuItem>
          <MenuItem value={DataSourceKind.TiDB}>TIDB</MenuItem>
          <MenuItem value={DataSourceKind.Postgres}>PostgreSQL</MenuItem>
          <MenuItem value={DataSourceKind.Sqlite}>SQLite</MenuItem>
          <MenuItem value={DataSourceKind.Redis}>Redis</MenuItem>
          <MenuItem value={DataSourceKind.Mongo}>Mongo</MenuItem>
          <MenuItem value={DataSourceKind.Oracle}>Oracle</MenuItem>
          <MenuItem value={DataSourceKind.SqlServer}>
            Sql Server(microsoft)
          </MenuItem>
          <MenuItem value={DataSourceKind.Cassandra}>Cassandra</MenuItem>
          <MenuItem value={DataSourceKind.ClickHouse}>Clickhouse</MenuItem>
          <MenuItem value={DataSourceKind.LibSql}> LibSql </MenuItem>
          <MenuItem value={DataSourceKind.FireBird}> FireBird </MenuItem>
          {/* <MenuItem value={DataSourceKind.DuckDB}> Duckdb </MenuItem> */}
          <MenuItem value={DataSourceKind.Cockroach}> Cockroach </MenuItem>
        </Select>
      </FormControl>
      <Stack direction="row" style={{ alignItems: 'flex-start' }} spacing={2}>
        <TextField
          label="Connection Name"
          value={connectionName}
          inputRef={connectionNameDom}
          required
          onChange={(e) => setConnectionName(e.target.value)}
          slotProps={{
            input: {
              endAdornment: connectionName && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setConnectionName('');
                    }}
                    edge="end"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
          style={{ flex: 1 }}
        />
        <TextField
          label="connection timeout"
          value={connTimeout}
          style={{ flex: 1 }}
          onChange={(e) => setConnTimeout(e.target.value)}
          slotProps={{
            input: {
              endAdornment: connTimeout && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setConnTimeout('');
                    }}
                    edge="end"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
          error={connTimeout ? isNaN(parseInt(connTimeout)) : false}
          helperText="milliseconds to wait for connect"
        />
        <FormControlLabel
          style={{ width: '180px' }}
          control={
            <Switch
              checked={readOnly}
              onChange={(e) => setReadOnly(e.target.checked)}
            />
          }
          label="Read Only"
        />
      </Stack>

      <AddConnectionContext
        value={{
          connectionName,
          connectionNameDom,
          handleClose,
          readOnly: !!readOnly,
          connTimeOut: connTimeout ? parseInt(connTimeout) : null,
        }}
      >
        <Stack
          style={{
            ...tabCss,
            border: '2px solid  ' + theme.palette.success.main,
          }}
          spacing={1}
        >
          <Typography variant="h6"> server info </Typography>
          {(connectionType === DataSourceKind.Mysql ||
            connectionType === DataSourceKind.MariaDB ||
            connectionType === DataSourceKind.TiDB) && (
            <Mysql
              config={config as types.MysqlDBConfig}
              kind={connectionType as DataSourceKind}
            ></Mysql>
          )}

          {connectionType === DataSourceKind.LibSql && (
            <LibSql config={config as types.LibSqlDBConfig}></LibSql>
          )}

          {connectionType === DataSourceKind.FireBird && (
            <FireBird config={config as types.FireBirdConfig}></FireBird>
          )}

          {(connectionType === DataSourceKind.Postgres ||
            connectionType === DataSourceKind.Cockroach) && (
            <PostGres
              config={config as types.PGDBConfig}
              kind={connectionType as DataSourceKind}
            ></PostGres>
          )}
          {connectionType === DataSourceKind.ClickHouse && (
            <ClickHouse config={config as types.ClickHouseConfig}></ClickHouse>
          )}

          {connectionType === DataSourceKind.Cassandra && (
            <Cassandra config={config as types.CassandraDBConfig}></Cassandra>
          )}

          {connectionType === DataSourceKind.Redis && (
            <Redis config={config as types.RedisDBConfig}></Redis>
          )}
          {connectionType === DataSourceKind.Mongo && (
            <MongoDb config={config as types.MongoDBConfig}></MongoDb>
          )}
          {connectionType === DataSourceKind.SqlServer && (
            <SqlServer config={config as types.SqlServerDBConfig}></SqlServer>
          )}
          {connectionType === DataSourceKind.Oracle && (
            <Oracle config={config as types.OracleDBConfig}></Oracle>
          )}
          {connectionType === DataSourceKind.Sqlite && (
            <Sqlite config={config as types.SqliteDBConfig}></Sqlite>
          )}
        </Stack>
      </AddConnectionContext>

      <TestConnAndSave
        handleClose={handleClose}
        connectionNameDom={connectionNameDom}
      ></TestConnAndSave>
    </Stack>
  );
}

function LibSql(props: { config?: types.LibSqlDBConfig }) {
  const { config } = props;
  const [url, setUrl] = useState(
    config && config.source == 'remote' && config.url
  );
  const [replica, setReplica] = useState(config?.replica);
  const [authToken, setAuthToken] = useState(
    config && config.source == 'remote' && config.auth_token
  );
  const [libSqlModel, setLibSqlModal] = useState(config?.source || 'remote');
  const [path, setPath] = useState(config?.path);
  const models = ['remote', 'local'];
  return (
    <>
      <RadioGroup
        row
        onChange={(e) => {
          setLibSqlModal(e.target.value as 'remote');
        }}
        value={libSqlModel}
      >
        {models.map((e) => (
          <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
        ))}
      </RadioGroup>

      {libSqlModel === 'local' && (
        <SelectPath
          path={path}
          setPath={setPath}
          required
          label="libsql database file"
        ></SelectPath>
      )}
      {libSqlModel === 'remote' && (
        <>
          <Stack direction="row" spacing={1}>
            <TextField
              value={url}
              label="url"
              onChange={(e) => {
                setUrl(e.target.value);
              }}
              fullWidth
              slotProps={{
                input: {
                  endAdornment: url && (
                    <InputAdornment position="end">
                      <IconButton
                        onClick={() => {
                          setUrl('');
                        }}
                        edge="end"
                      >
                        <ClearIcon fontSize="small" />
                      </IconButton>
                    </InputAdornment>
                  ),
                },
              }}
            ></TextField>
            <TextField
              fullWidth
              value={authToken}
              label="auth token "
              onChange={(e) => {
                setAuthToken(e.target.value);
              }}
              slotProps={{
                input: {
                  endAdornment: authToken && (
                    <InputAdornment position="end">
                      <IconButton
                        onClick={() => {
                          setAuthToken('');
                        }}
                        edge="end"
                      >
                        <ClearIcon fontSize="small" />
                      </IconButton>
                    </InputAdornment>
                  ),
                },
              }}
            ></TextField>
          </Stack>
          <Stack
            direction="row"
            style={{ alignItems: 'center' }}
            spacing={1}
            flex={1}
          >
            <Checkbox
              value={replica}
              onChange={(e) => {
                setReplica(e.target.checked);
              }}
            ></Checkbox>
            replica
            {replica && (
              <SelectPath
                label="replica"
                path={path}
                setPath={setPath}
                required={false}
              ></SelectPath>
            )}
          </Stack>
        </>
      )}
    </>
  );
}

export { AddConnection };
