import { useContext, useEffect, useState } from 'react';

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
  CircularProgress,
  Alert,
} from '@mui/material';
import { NotificationContext } from './Notification';
import { types } from './ApiStruct';

import { invoke } from '@tauri-apps/api/core';
import { DataSourceKind, msgTimeout } from './Common';
import { open } from '@tauri-apps/plugin-dialog';

import AttachFileIcon from '@mui/icons-material/AttachFile';

async function selectFile() {
  const selected = await open({
    multiple: false, // 是否允许选择多个文件
  });
  return selected;
}

function HostAndPort(props: { host; port; setHost; setPort }) {
  let { host, port, setHost, setPort } = props;
  return (
    <Stack direction='row'>
      <TextField
        margin='normal'
        fullWidth
        label='Host'
        value={host}
        size='small'
        sx={{ mr: 1 }}
        onChange={(e) => setHost(e.target.value)}
      />
      <TextField
        margin='normal'
        fullWidth
        label='port'
        error={isNaN(parseInt(port))}
        size='small'
        value={port}
        onChange={(e) => setPort(e.target.value)}
      />
    </Stack>
  );
}

function Sqlite(props: { path; setPath }) {
  let { path, setPath } = props;
  return (
    <Stack>
      <Typography>path:{path}</Typography>
      <Button
        onClick={() => {
          selectFile().then((e) => {
            setPath(e);
          });
        }}
      >
        <AttachFileIcon></AttachFileIcon>
      </Button>
    </Stack>
  );
}

function UserAndPassword(props: { user; password; setUser; setPassword }) {
  let { user, password, setUser, setPassword } = props;

  return (
    <Stack direction='row'>
      <TextField
        margin='normal'
        fullWidth
        label='User'
        value={user}
        sx={{ mr: 1 }}
        size='small'
        onChange={(e) => setUser(e.target.value)}
      />
      <TextField
        margin='normal'
        fullWidth
        label='Password'
        size='small'
        type='password'
        value={password}
        onChange={(e) => setPassword(e.target.value)}
      />
    </Stack>
  );
}

function EnableSSL(props: { useSSL; setUseSSL }) {
  let { useSSL, setUseSSL } = props;
  return (
    <FormControlLabel
      control={
        <Switch
          checked={useSSL}
          onChange={(e) => setUseSSL(e.target.checked)}
        />
      }
      label='Enable SSL'
    />
  );
}

function TrustCert(props: { trustCert; setTrustCert }) {
  let { trustCert, setTrustCert } = props;
  return (
    <FormControlLabel
      control={
        <Switch
          checked={trustCert}
          onChange={(e) => setTrustCert(e.target.checked)}
        />
      }
      label='Trust Cert'
    />
  );
}

function Database(props: { database; setDatabase }) {
  let { database, setDatabase } = props;
  return (
    <Stack direction='row'>
      <TextField
        margin='normal'
        label='database'
        type='database'
        size='small'
        value={database}
        onChange={(e) => setDatabase(e.target.value)}
      />
    </Stack>
  );
}

export default function AddConfig() {
  const [connectionType, setConnectionType] = useState(DataSourceKind.Mysql);
  const [connectionName, setConnectionName] = useState('');
  const [host, setHost] = useState('localhost');
  const [port, setPort] = useState('3306');
  const [user, setUser] = useState('');
  const [password, setPassword] = useState('');
  const [database, setDatabase] = useState('');
  const [readOnly, setReadOnly] = useState(false);
  const [useSSL, setUseSSL] = useState(false);
  const [testLoading, setTestLoading] = useState(false);
  const [path, setPath] = useState('');
  const { showNotification } = useContext(NotificationContext);
  const [testOk, setTestOk] = useState(false);
  const [trustCert, setTrustCert] = useState(false);
  const makeConfig = (): types.DBConnectionConfig | null => {
    let portInt = parseInt(port);
    if (isNaN(portInt)) {
      showNotification('error', 'port is not a number');
      return null;
    }
    let user_null = user ? user : null;
    let pass_null = password ? password : null;
    let db_null = database ? database : null;
    switch (connectionType) {
      case DataSourceKind.Mysql:
        return {
          conn_type: 'mysql',
          host: host,
          port: portInt,
          username: user,
          password: pass_null,
          db: db_null,
          read_only: readOnly,
          name: connectionName,
        };
      case DataSourceKind.MariaDB:
        return {
          conn_type: 'mariadb',
          host: host,
          port: portInt,
          username: user,
          password: pass_null,
          db: db_null,
          read_only: readOnly,
          name: connectionName,
        };
      case DataSourceKind.Postgres:
        return {
          conn_type: 'postgres',
          host: host,
          port: portInt,
          username: user,
          password: pass_null,
          db: db_null,
          read_only: readOnly,
          name: connectionName,
          schema: null,
        };

      case DataSourceKind.Sqlite:
        return {
          conn_type: 'sqlite',
          path: path,
          read_only: readOnly,
          name: connectionName,
        };
      case DataSourceKind.Redis:
        return {
          conn_type: 'redis',
          read_only: readOnly,
          name: connectionName,
          host: host,
          port: portInt,
          db: db_null,
          username: user_null,
          password: pass_null,
        };
      case DataSourceKind.Mongo:
        return {
          conn_type: 'mongo',
          read_only: readOnly,
          name: connectionName,
          host: host,
          port: portInt,
          db: db_null,
          username: user_null,
          password: pass_null,
          connect_timeout: null,
        };
      case DataSourceKind.MsSql:
        return {
          conn_type: 'sqlserver',
          read_only: readOnly,
          name: connectionName,
          host: host,
          port: portInt,
          db: db_null,
          username: user,
          password: password,
          trust_cert: trustCert,
          schema: null,
        };
    }
  };
  const handleTestConnection = () => {
    const config = makeConfig();
    if (config) {
      setTestLoading(true);
      invoke('test_connection', { config })
        .then(() => {
          setTestLoading(false);
          setTestOk(true);
          setTimeout(() => {
            setTestOk(false);
          }, msgTimeout);
        })
        .catch((e) => {
          setTestLoading(false);
          showNotification('error', 'test connection failed', e);
        });
    }
  };

  useEffect(() => {
    switch (connectionType) {
      case DataSourceKind.MariaDB:
      case DataSourceKind.Mysql:
        setPort('3306');
        break;
      case DataSourceKind.Sqlite:
        break;
      case DataSourceKind.Postgres:
        setPort('5432');
        break;
      case DataSourceKind.MsSql:
        setPort('1433');
        break;
      case DataSourceKind.MsSql:
        setPort('1433');
        break;
      case DataSourceKind.Redis:
        setPort('6379');
        break;
      case DataSourceKind.Mongo:
        setPort('27017');
        break;
    }
  }, [connectionType]);
  const handleSaveConnection = () => {
    if (!connectionName) {
      showNotification('error', 'Connection Name is empty.');
      return;
    }
    const config = makeConfig();
    if (!config) {
      return;
    }
    invoke('add_config', { config })
      .then((_) => {
        showNotification('success', 'Add Connection OK');
      })
      .catch((e) => {
        showNotification('error', 'Add Connection failure', e);
      });
  };
  return (
    <Stack spacing={1}>
      <Typography variant='h6' gutterBottom>
        New Connection
      </Typography>
      <FormControl fullWidth margin='normal'>
        <InputLabel>Connection Type</InputLabel>
        <Select
          size='small'
          value={connectionType}
          label='Connection Type'
          onChange={(e) => setConnectionType(e.target.value as DataSourceKind)}
        >
          <MenuItem value={DataSourceKind.Mysql}>MySQL</MenuItem>
          <MenuItem value={DataSourceKind.MariaDB}>MariaDB</MenuItem>
          <MenuItem value={DataSourceKind.Postgres}>PostgreSQL</MenuItem>
          <MenuItem value={DataSourceKind.Sqlite}>SQLite</MenuItem>
          <MenuItem value={DataSourceKind.Redis}>Redis</MenuItem>
          <MenuItem value={DataSourceKind.Mongo}>Mongo</MenuItem>
          <MenuItem value={DataSourceKind.MsSql}>
            Sql server(microsoft)
          </MenuItem>
        </Select>
      </FormControl>

      {(connectionType == DataSourceKind.Mysql ||
        connectionType == DataSourceKind.MariaDB) && (
        <>
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
          ></UserAndPassword>
          <Database database={database} setDatabase={setDatabase}></Database>
        </>
      )}
      {connectionType == DataSourceKind.Postgres && (
        <>
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
          ></UserAndPassword>
          <Database database={database} setDatabase={setDatabase}></Database>
        </>
      )}
      {connectionType == DataSourceKind.Redis && (
        <>
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
          ></UserAndPassword>
          <Database database={database} setDatabase={setDatabase}></Database>
        </>
      )}
      {connectionType == DataSourceKind.Mongo && (
        <>
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
          ></UserAndPassword>
          <Database database={database} setDatabase={setDatabase}></Database>
        </>
      )}
      {connectionType == DataSourceKind.MsSql && (
        <>
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
          ></UserAndPassword>
          <Database database={database} setDatabase={setDatabase}></Database>
          <TrustCert
            trustCert={trustCert}
            setTrustCert={setTrustCert}
          ></TrustCert>
        </>
      )}
      {connectionType == DataSourceKind.Sqlite && (
        <Sqlite path={path} setPath={setPath}></Sqlite>
      )}

      <Stack direction='row'>
        <Button
          variant='outlined'
          sx={{ mr: 1 }}
          onClick={handleTestConnection}
        >
          Test
        </Button>
        {testOk && <Alert severity='success'> Test OK </Alert>}
        {testLoading && <CircularProgress />}
      </Stack>
      <FormControlLabel
        control={
          <Switch
            checked={readOnly}
            onChange={(e) => setReadOnly(e.target.checked)}
          />
        }
        label='Read Only'
      />
      <Stack direction='row' spacing={1} alignContent='flex-start'>
        <TextField
          margin='normal'
          label='Connection Name'
          value={connectionName}
          error={!connectionName}
          required
          size='small'
          onChange={(e) => setConnectionName(e.target.value)}
          helperText='Enter a name for this connection'
        />

        <Button
          variant='contained'
          color='primary'
          size='small'
          onClick={() => {
            handleSaveConnection();
          }}
        >
          Save
        </Button>
      </Stack>
    </Stack>
  );
}
