import {
  Dispatch,
  RefObject,
  SetStateAction,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { AddIcon } from "./icons";
import {
  TextField,
  Button,
  FormControlLabel,
  Typography,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Stack,
  Divider,
  Checkbox,
  Chip,
  Tooltip,
  InputAdornment,
  IconButton,
  Autocomplete,
  Slider,
  Alert,
} from "@mui/material";
import { CancelIcon } from "./icons";
import { ClearableTextField, RefreshButton } from "./CommonComp2";
import { VisibilityIcon } from "./icons";
import { message, open } from "@tauri-apps/plugin-dialog";
import { extractBaseName } from "./Common.ts";
import { DataSourceKind } from "./helper.ts";
import { openUrl } from "@tauri-apps/plugin-opener";
import { platform } from "@tauri-apps/plugin-os";
import { StackRowCenter, UrlDocument } from "./CommonComp2.tsx";
import { useConnectionStore, useNotification } from "./store.tsx";
import {
  commands,
  DBConnectionConfig,
  MysqlDBConfig,
  OdbcCommon,
  PGDBConfig,
  SSLFiles,
  OracleDBConfig,
  ClickHouseConfig,
  SqliteDBConfig,
  FireBirdConfig,
  RedisDBConfig,
  MongoDBConfig,
  CassandraDBConfig,
  SqlServerDBConfig,
  DuckDBConfig,
  LibSqlDBConfig,
  Db2Config,
  CouchBaseDBConfig,
  CassandraDialect,
  SslFilesWithStrModel,
} from "./bindings.ts";
import { SelectPath } from "./CommonComp.tsx";

/**
 * Host and port input pair used by several connection forms.
 */
function HostAndPort(props: {
  host: string;
  port: string;
  setHost: Dispatch<SetStateAction<string>>;
  setPort: Dispatch<SetStateAction<string>>;
}) {
  const { host, port, setHost, setPort } = props;
  return (
    <Stack direction="row">
      <ClearableTextField
        fullWidth
        label="Host"
        value={host}
        sx={{ mr: 1 }}
        onClear={() => {
          setHost("");
        }}
        onChange={(event) => setHost(event.target.value)}
        required
      />
      <TextField
        fullWidth
        label="port"
        error={isNaN(parseInt(port))}
        value={port}
        onChange={(event) => setPort(event.target.value)}
      />
    </Stack>
  );
}

/**
 * User and password fields with optional password visibility and save option.
 */
function UserAndPassword(props: {
  user: string;
  password: string;
  setUser: Dispatch<SetStateAction<string>>;
  setPassword: Dispatch<SetStateAction<string>>;
  savePassword: boolean;
  setSavePassword: Dispatch<SetStateAction<boolean>>;
}) {
  const { user, password, setUser, setPassword, savePassword, setSavePassword } = props;
  const [passwordVisible, setPasswordVisible] = useState(false);
  return (
    <Stack direction="row">
      <ClearableTextField
        fullWidth
        label="User"
        value={user}
        sx={{ mr: 1 }}
        onChange={(event) => setUser(event.target.value)}
        onClear={() => setUser("")}
      />
      <TextField
        fullWidth
        label="Password"
        type={passwordVisible ? "text" : "password"}
        value={password}
        onChange={(event) => setPassword(event.target.value)}
        slotProps={{
          input: {
            endAdornment: !passwordVisible &&
              password != "" &&
              platform() != "windows" && ( // Windows already has this feature.
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setPasswordVisible(true);
                    }}
                    edge="end">
                    <VisibilityIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
          },
        }}
      />
      <Stack
        style={{
          width: "31rem",
          justifyContent: "center",
          alignItems: "center",
        }}
        direction="row">
        <Checkbox
          checked={savePassword}
          onChange={(event) => {
            setSavePassword(event.target.checked);
          }}
          name="save password"
        />
        save password
      </Stack>
    </Stack>
  );
}

/**
 * HTTPS toggle used by connection types that support secure HTTP.
 */
function Https(props: { https: boolean; setHttps: Dispatch<SetStateAction<boolean>> }) {
  const { https, setHttps } = props;
  return (
    <FormControlLabel
      control={
        <Checkbox
          checked={https}
          onChange={(event) => setHttps(event.target.checked)}
        />
      }
      label="HTTPS"
    />
  );
}

/**
 * SQL Server-specific TLS certificate configuration controls.
 */
function SqlServerCert(props: {
  data: SqlServerCertData;
  setData: Dispatch<SetStateAction<SqlServerCertData>>;
}) {
  const { data, setData } = props;
  return (
    <StackRowCenter>
      <FormControlLabel
        control={
          <Checkbox
            checked={data.trust_cert}
            onChange={(e) =>
              setData({
                ...data,
                trust_cert: e.target.checked,
              })
            }
          />
        }
        label="Trust Cert"
      />
      <Chip
        label="?"
        onClick={() => {
          openUrl(
            "https://docs.rs/tiberius/0.12.3/tiberius/struct.Config.html#method.trust_cert",
          );
        }}
      />
    </StackRowCenter>
  );
}

/**
 * SQL Server certificate settings used for encrypted connections.
 */
type SqlServerCertData = {
  trust_cert: boolean;
  cert_ca: string | null;
};

/**
 * Certificate file selectors for SSL/TLS-enabled connections.
 */
function SslFiles(props: {
  files: SSLFiles;
  setFiles: Dispatch<SetStateAction<SSLFiles>>;
  enabled: boolean;
}) {
  const { files, setFiles, enabled } = props;

  return (
    enabled && (
      <>
        <Stack direction="row" style={{ flexWrap: "wrap" }}>
          <Tooltip
            title={files.ssl_ca}
            onClick={async () => {
              const file = await open({ multiple: false });
              if (file) {
                setFiles({ ...files, ssl_ca: file });
              }
            }}>
            <Chip
              label={"CA Cert:" + (files.ssl_ca ? extractBaseName(files.ssl_ca) : "")}
              variant="outlined"
            />
          </Tooltip>
          <Tooltip title={files.ssl_cert}>
            <Chip
              variant="outlined"
              onClick={async () => {
                const file = await open({ multiple: false });
                if (file) {
                  setFiles({ ...files, ssl_cert: file });
                }
              }}
              label={
                "Certificate:" + (files.ssl_cert ? extractBaseName(files.ssl_cert) : "")
              }
            />
          </Tooltip>
          <Tooltip title={files.ssl_key}>
            <Chip
              variant="outlined"
              onClick={async () => {
                const file = await open({ multiple: false });
                if (file) {
                  setFiles({ ...files, ssl_key: file });
                }
              }}
              label={
                "Key File:" + (files.ssl_key ? extractBaseName(files.ssl_key) : "")
              }
            />
          </Tooltip>
        </Stack>
      </>
    )
  );
}

function PGSSL(props: {
  ssl: SslFilesWithStrModel;
  setSsl: Dispatch<SetStateAction<SslFilesWithStrModel>>;
}) {
  const { ssl, setSsl } = props;
  const modes: string[] = [
    "disable",
    "allow",
    "prefer",
    "require",
    "verify-ca",
    "verify-full",
  ];

  return (
    <>
      <SSLNotice backend="native-tls" />
      <Stack direction="row" style={{ alignItems: "center", flexWrap: "wrap" }}>
        <FormControl style={{ width: "12rem" }}>
          <InputLabel> ssl mode </InputLabel>
          <Select
            value={ssl.ssl_mode}
            label="ssl mode"
            onChange={(e) => setSsl({ ...ssl, ssl_mode: e.target.value })}>
            {modes.map((mode) => (
              <MenuItem value={mode} key={mode}>
                {mode}
              </MenuItem>
            ))}
          </Select>
        </FormControl>
        <UrlDocument url="https://docs.rs/sqlx/latest/sqlx/postgres/enum.PgSslMode.html" />

        <SslFiles enabled={ssl.ssl_mode != "disable"} files={ssl} setFiles={setSsl} />
      </Stack>
    </>
  );
}
function MysqlSSl(props: {
  ssl: SslFilesWithStrModel;
  setSsl: Dispatch<SetStateAction<SslFilesWithStrModel>>;
}) {
  const { ssl, setSsl } = props;
  const models: string[] = [
    "disabled",
    "preferred",
    "required",
    "verify_ca",
    "verify_identity",
  ];
  return (
    <>
      <SSLNotice backend="native-tls" />
      <Stack direction="row" style={{ alignItems: "center", flexWrap: "wrap" }}>
        <FormControl style={{ width: "12rem" }}>
          <InputLabel> ssl mode </InputLabel>
          <Select
            value={ssl.ssl_mode}
            label="ssl mode"
            onChange={(e) => setSsl({ ...ssl, ssl_mode: e.target.value })}>
            {models.map((e) => (
              <MenuItem value={e} key={e}>
                {e}
              </MenuItem>
            ))}
          </Select>
        </FormControl>

        <UrlDocument url="https://docs.rs/sqlx/latest/sqlx/mysql/enum.MySqlSslMode.html" />
        <SslFiles enabled={ssl.ssl_mode != "disabled"} files={ssl} setFiles={setSsl} />
      </Stack>
    </>
  );
}

/**
 * Autocomplete field for database/schema candidates.
 *
 * @param props.name current selected value
 * @param props.setName updates the selected candidate name
 * @param props.label label text shown in the input field
 * @param props.makeConfig callback to build the current connection config
 */
function DbSchemaCadidates(props: {
  name: string;
  setName: Dispatch<SetStateAction<string>>;
  label: string;
  makeConfig: MakeConfig;
}) {
  const { name, setName, label, makeConfig } = props;
  const [candidates, setCadidates] = useState<string[]>([]);
  const { showNotification } = useNotification();
  const [loading, setLoading] = useState(false);
  const handleReload = () => {
    const config = makeConfig();
    if (!config.name) {
      config.name = "test_conn";
    }
    setLoading(true);
    commands
      .tryListDatabaseSchemaCandidates(config)
      .then((data) => {
        setCadidates(data);
        showNotification({ severity: "success", title: "fetch candidates success" });
      })
      .catch((e) => {
        showNotification({
          severity: "warning",
          title: `list ${label} failed`,
          message: e,
        });
      })
      .finally(() => {
        setLoading(false);
      });
  };
  return (
    <Stack direction="row" style={{ alignItems: "stretch" }}>
      <Autocomplete
        options={candidates}
        style={{ width: "22rem" }}
        value={name}
        renderInput={(params) => (
          <TextField {...params} label={`${label} candidates`} />
        )}
        freeSolo
        onChange={(_, name) => {
          setName((name ?? "") as string);
        }}
        onInputChange={(_, value) => {
          setName(value);
        }}
      />
      <RefreshButton onClick={handleReload} loading={loading} />
    </Stack>
  );
}

/**
 * Helper type for generating the current connection configuration.
 */
type MakeConfig = () => DBConnectionConfig;

function Mysql(props: {
  config?: MysqlDBConfig;
  kind: DataSourceKind;
  context: AddConnectionContext;
}) {
  const { config, kind, context } = props;
  const { connectionName, connTimeOut, readOnly, makeConfigRef } = context;

  let defaultPort = 3306;
  if (kind == DataSourceKind.TiDB) {
    defaultPort = 4000;
  }
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || defaultPort) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [database, setDatabase] = useState(getDB(config));
  const [ssl, setSsl] = useState<SslFilesWithStrModel>({
    ssl_mode: config?.ssl_mode || "preferred",
    ssl_ca: config?.ssl_ca,
    ssl_cert: config?.ssl_cert,
    ssl_key: config?.ssl_key,
  });
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const [charset, setCharset] = useState(config?.charset || "utf8mb4");

  function makeConfig(): DBConnectionConfig {
    const x: MysqlDBConfig = {
      host,
      port: parseInt(port),
      username: user,
      password: password ? password : null,
      db: database ? database : null,
      read_only: readOnly,
      charset,
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
          save_password: savePassword,
          ...x,
        };
      default:
        message("pass wrong kind of parameter, kind:" + kind, {
          kind: "error",
        });
    }
  }
  makeConfigRef.current = makeConfig;

  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <DbSchemaCadidates
        name={database}
        label="database"
        setName={setDatabase}
        makeConfig={makeConfig}
      />
      <MysqlSSl ssl={ssl} setSsl={setSsl} />
      <TextField
        value={charset}
        onChange={(e) => {
          setCharset(e.target.value);
        }}
        error={!charset}
        label="charset"
      />
    </Stack>
  );
}

function Oracle(props: { config?: OracleDBConfig; context: AddConnectionContext }) {
  const { config, context } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || 1521) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [database, setDatabase] = useState(config?.db);
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const [schema, setSchema] = useState(getSchema(config));
  const { connTimeOut, connectionName, readOnly, makeConfigRef } = context;

  function makeConfig(): DBConnectionConfig {
    return {
      conn_type: "oracle",
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
  makeConfigRef.current = makeConfig;

  return (
    <Stack>
      <Typography
        color="info"
        style={{ cursor: "pointer" }}
        onClick={() => {
          openUrl(
            "https://odpi-c.readthedocs.io/en/latest/user_guide/installation.html",
          );
        }}>
        oracle need extra driver
      </Typography>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <TextField
        value={database}
        label="database"
        onChange={(e) => {
          setDatabase(e.target.value);
        }}
      />
      {database && (
        <DbSchemaCadidates
          name={schema}
          setName={setSchema}
          makeConfig={makeConfig}
          label="schema"
        />
      )}
    </Stack>
  );
}

function ClickHouse(props: {
  config?: ClickHouseConfig;
  context: AddConnectionContext;
}) {
  const { config, context } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || 8123) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [database, setDatabase] = useState(getDB(config));
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const [https, setHttps] = useState(config?.https);
  const { connTimeOut, connectionName, readOnly, makeConfigRef } = context;

  function makeConfig(): DBConnectionConfig {
    return {
      conn_type: "clickhouse",
      name: connectionName,
      host,
      port: parseInt(port),
      username: user,
      password: password ? password : null,
      save_password: savePassword,
      db: database ? database : null,
      https: !!https,
      read_only: readOnly,
      conn_timeout: connTimeOut,
    };
  }
  makeConfigRef.current = makeConfig;

  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        makeConfig={makeConfig}
        label="database"
      />
      <Https https={https} setHttps={setHttps} />
    </Stack>
  );
}

type PostGresOption = {
  key: string;
  value: string;
};

function PostGresOptions(props: {
  options: PostGresOption[];
  setOptions: Dispatch<SetStateAction<PostGresOption[]>>;
}) {
  const { options, setOptions } = props;

  return (
    <Stack style={{ border: "1px solid grey", borderRadius: "0.4rem", padding: "3px" }}>
      <Stack direction="row" style={{ alignItems: "center" }}>
        <IconButton
          color="primary"
          onClick={() => {
            setOptions([...options, { key: "", value: "" }]);
          }}>
          <AddIcon />
        </IconButton>
        <Typography> extra options </Typography>
        <UrlDocument url="https://docs.rs/sqlx/latest/sqlx/postgres/struct.PgConnectOptions.html#method.options" />
      </Stack>
      {options.map((value, index) => (
        <Stack key={index} direction="row" style={{ alignItems: "center" }}>
          <IconButton
            onClick={() => {
              setOptions(options.filter((_, idx) => index != idx));
            }}>
            <CancelIcon />
          </IconButton>
          <TextField
            fullWidth
            value={value.key}
            onChange={(event) => {
              setOptions(
                options.map((op, idx) => {
                  if (index == idx) {
                    op.key = event.target.value;
                  }
                  return op;
                }),
              );
            }}
            label="key"
          />
          <TextField
            label="value"
            fullWidth
            value={value.value}
            onChange={(event) => {
              setOptions(
                options.map((op, idx) => {
                  if (index == idx) {
                    op.value = event.target.value;
                  }
                  return op;
                }),
              );
            }}
          />
        </Stack>
      ))}
    </Stack>
  );
}

function PostGres(props: {
  config?: PGDBConfig;
  kind: DataSourceKind;
  context: AddConnectionContext;
}) {
  const { showNotification } = useNotification();
  const { config, kind, context } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState<string>(
    (config?.port || (kind === DataSourceKind.Postgres ? 5432 : 26257)) + "",
  );

  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [database, setDatabase] = useState(getDB(config));
  const [ssl, setSSl] = useState<SslFilesWithStrModel>({
    ssl_mode: config?.ssl_mode || "prefer",
    ssl_ca: config?.ssl_ca,
    ssl_cert: config?.ssl_cert,
    ssl_key: config?.ssl_key,
  });
  const [schema, setSchema] = useState(getSchema(config));
  const { connTimeOut, connectionName, readOnly, makeConfigRef } = context;
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const [options, setOptions] = useState<PostGresOption[]>(
    (function () {
      if (!config) {
        return [];
      }
      const ret: PostGresOption[] = [];
      for (const key in config.options) {
        ret.push({ key, value: config.options[key] });
      }
      return ret;
    })(),
  );
  function makeConfig(): DBConnectionConfig {
    const x: PGDBConfig = {
      host,
      port: parseInt(port),
      username: user,
      password: password ? password : null,
      db: database ? database : null,
      read_only: readOnly,
      conn_timeout: connTimeOut,
      schema: schema ? schema : null,
      options: (function () {
        const ret = {};
        for (const op of options) {
          if (!op.key) {
            continue;
          }
          if (op.key in ret) {
            showNotification({
              severity: "error",
              title: `key '${op.key}' duplicated`,
            });
            return;
          }
          ret[op.key] = op.value;
        }
        return ret;
      })(),
      ...ssl,
    };
    switch (kind) {
      case DataSourceKind.Postgres:
      case DataSourceKind.Cockroach:
        return {
          conn_type: kind,
          name: connectionName,
          save_password: savePassword,
          ...x,
        };
      default:
        message("pass wrong kind of parameter, kind:" + kind, {
          kind: "error",
        });
    }
  }

  makeConfigRef.current = makeConfig;
  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        makeConfig={makeConfig}
        label="database"
      />
      {database && (
        <DbSchemaCadidates
          name={schema}
          setName={setSchema}
          label="schema"
          makeConfig={makeConfig}
        />
      )}
      <PGSSL ssl={ssl} setSsl={setSSl} />
      <PostGresOptions options={options} setOptions={setOptions} />
    </Stack>
  );
}

function Sqlite(props: { config?: SqliteDBConfig; context: AddConnectionContext }) {
  const { config, context } = props;
  const [path, setPath] = useState(config?.path || "");
  const { connTimeOut, connectionName, readOnly, makeConfigRef } = context;
  const [lockingModel, setLockingModel] = useState(config?.locking_model || "normal");

  function makeConfig(): DBConnectionConfig {
    return {
      conn_type: "sqlite",
      name: connectionName,
      path,
      conn_timeout: connTimeOut,
      read_only: readOnly,
      locking_model: lockingModel,
    };
  }
  makeConfigRef.current = makeConfig;

  return (
    <Stack>
      <SelectPath path={path} setPath={setPath} required label="sqlite database file" />
      <SqliteLockingModel
        lockingModel={lockingModel}
        setLockingModel={setLockingModel}
      />
    </Stack>
  );
}

function SqliteLockingModel({ lockingModel, setLockingModel }) {
  return (
    <Stack direction="row" style={{ alignItems: "center", alignContent: "center" }}>
      <Autocomplete
        options={["normal", "exclusive"]}
        value={lockingModel}
        renderInput={(params) => (
          <TextField {...params} label={`sqlite locking model`} />
        )}
        style={{ width: "12rem" }}
        freeSolo={false}
        onChange={(_, value) => {
          setLockingModel(value);
        }}
      />
      <UrlDocument url="https://www.sqlite.org/pragma.html#pragma_locking_mode" />
    </Stack>
  );
}

/**
 * Shared context passed to each backend-specific connection form.
 *
 * - connectionName: current name of the connection
 * - readOnly: whether the connection should be opened read-only
 * - connTimeOut: connection timeout in milliseconds
 * - connectionNameDom: ref to the connection name input field
 * - makeConfigRef: ref used to expose the current makeConfig() function
 */
type AddConnectionContext = {
  connectionName: string;
  readOnly: boolean;
  connTimeOut: number | null;
  connectionNameDom: RefObject<HTMLInputElement | null>;
  makeConfigRef: RefObject<MakeConfig | null>;
};

// global var make it simple the pass the parameter
// add connection run in separete window
// let globalMakeConfig: MakeConfig = null;

/**
 * Buttons for testing and saving the current connection configuration.
 */
function TestConnAndSave(props: {
  connectionNameDom: RefObject<HTMLInputElement>;
  makeConfig: () => DBConnectionConfig;
  cnameDuplicated: boolean;
}) {
  const { connectionNameDom, makeConfig, cnameDuplicated } = props;
  const [testLoading, setTestLoading] = useState(false);
  const { showNotification } = useNotification();

  const { addConnection } = useConnectionStore();

  const handleTestConnection = () => {
    const config = makeConfig();
    if (!config.name) {
      config.name = "test_connection";
    }
    if (config) {
      console.info("test config:", config);
      setTestLoading(true);
      commands
        .testConnection(config)
        .then(() => {
          showNotification({ severity: "success", title: "test connection ok" });
        })
        .catch((e) => {
          showNotification({
            severity: "warning",
            title: "test connection failed",
            message: e,
          });
        })
        .finally(() => {
          setTestLoading(false);
        });
    }
  };

  const handleSaveConnection = () => {
    const config = makeConfig();
    if (!config) {
      return;
    }
    if (!config.name) {
      connectionNameDom.current?.focus();
      showNotification({ severity: "warning", title: "connection name is empty." });
      return;
    }
    if (config.name.includes("/")) {
      showNotification({
        severity: "error",
        title: `connection name can't  contains '/'`,
      });
      connectionNameDom.current?.focus();
      return;
    }
    addConnection(config)
      .then(() => {
        showNotification({ severity: "success", title: "add connection OK" });
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "add connection failure",
          message: e,
        });
      });
  };
  return (
    <Stack>
      <Divider />
      <StackRowCenter>
        <Button
          color="success"
          onClick={() => {
            handleSaveConnection();
          }}
          disabled={cnameDuplicated}>
          save
        </Button>
        <Button
          variant="outlined"
          sx={{ mr: 1 }}
          onClick={handleTestConnection}
          loading={testLoading}>
          test
        </Button>
      </StackRowCenter>
    </Stack>
  );
}

/**
 * Helper functions to safely extract common fields from a backend config object.
 */
/**
 * Safe field extractors for optional backend config objects.
 */
function getHost(config?: Record<string, unknown>) {
  return typeof config?.host === "string" ? config.host : "127.0.0.1";
}

function getUser(config?: Record<string, unknown>) {
  return typeof config?.username === "string" ? config.username : "";
}
function getPassword(config?: Record<string, unknown>) {
  return typeof config?.password === "string" ? config.password : "";
}
function getSavePassword(config?: Record<string, unknown>) {
  return typeof config?.save_password === "boolean" ? config.save_password : true;
}

function getDB(config?: Record<string, unknown>) {
  return typeof config?.db === "string" ? config.db : "";
}
function getSchema(config?: Record<string, unknown>) {
  return typeof config?.schema === "string" ? config.schema : "";
}

function FireBird(props: { config?: FireBirdConfig; context: AddConnectionContext }) {
  const { config, context } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || 3050) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const { connTimeOut, connectionName, readOnly, makeConfigRef } = context;
  const [database, setDatabase] = useState(getDB(config));
  const [dialect, setDialect] = useState("3");

  function makeConfig(): DBConnectionConfig {
    return {
      conn_type: "firebird",
      dialect,
      name: connectionName,
      host,
      port: parseInt(port),
      username: user,
      password,
      save_password: savePassword,
      db: database ? database : null,
      conn_timeout: connTimeOut,
      read_only: readOnly,
    };
  }
  makeConfigRef.current = makeConfig;

  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <TextField
        label="database"
        value={database}
        onChange={(e) => {
          setDatabase(e.target.value);
        }}
      />

      <Autocomplete
        options={["1", "2", "3"]}
        style={{ width: "22rem" }}
        value={dialect}
        renderInput={(params) => <TextField {...params} label={`dialect`} />}
        freeSolo={false}
        onChange={(_, value) => {
          setDialect(value);
        }}
      />
    </Stack>
  );
}

function Redis(props: { config?: RedisDBConfig; context: AddConnectionContext }) {
  const { config, context } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || 6379) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config) + "");
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const [database, setDatabase] = useState(getDB(config));
  const { connectionName, connTimeOut, readOnly, makeConfigRef } = context;
  const [enableSSL, setEnableSSL] = useState(!!config?.enable_ssl);
  function makeConfig(): DBConnectionConfig {
    return {
      name: connectionName,
      conn_type: "redis",
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

  makeConfigRef.current = makeConfig;

  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        label="database"
        makeConfig={makeConfig}
      />
      <SSLNotice backend="native-tls" />
      <FormControlLabel
        control={
          <Checkbox
            checked={enableSSL}
            onChange={(e) => setEnableSSL(e.target.checked)}
          />
        }
        label="enable ssl"
      />
    </Stack>
  );
}

function MongoDb(props: { config?: MongoDBConfig; context: AddConnectionContext }) {
  const { config, context } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || 27017) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const [database, setDatabase] = useState(getDB(config));
  const [enableSSL, setEnableSSL] = useState(false);
  const { connTimeOut, connectionName, readOnly, makeConfigRef } = context;
  const [accInvalidHosts, setAccInvalidHosts] = useState(false);
  const [accInvalidCerts, setAccInvalidCerts] = useState(false);

  function makeConfig(): DBConnectionConfig {
    return {
      conn_type: "mongo",
      name: connectionName,
      read_only: readOnly,
      host,
      username: user ? user : null,
      password: password ? password : null,
      conn_timeout: connTimeOut,
      port: parseInt(port),
      save_password: savePassword,
      ssl_enable: enableSSL,
      db: database ? database : null,
      allow_invalid_certificates: accInvalidCerts,
      allow_invalid_hostnames: accInvalidHosts,
    };
  }
  makeConfigRef.current = makeConfig;
  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        label="database"
        makeConfig={makeConfig}
      />

      <Stack direction="row" style={{ alignItems: "center" }}>
        <FormControlLabel
          style={{ width: "180px" }}
          control={
            <Checkbox
              checked={enableSSL}
              onChange={(e) => setEnableSSL(e.target.checked)}
            />
          }
          label="Enable SSL"
        />
        {enableSSL && (
          <AccInvalidHostNameAndCerts
            accInvalidCerts={accInvalidCerts}
            setAccInvalidHosts={setAccInvalidHosts}
            accInvalidHosts={accInvalidHosts}
            setAccInvalidCerts={setAccInvalidCerts}
          />
        )}
      </Stack>
      {enableSSL && <SSLNotice backend="native-tls" />}
    </Stack>
  );
}

function Cassandra(props: {
  config?: CassandraDBConfig;
  context: AddConnectionContext;
  dialect: CassandraDialect;
}) {
  const { config, context, dialect } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || 9042) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const [database, setDatabase] = useState("");
  const { connTimeOut, connectionName, readOnly, makeConfigRef } = context;
  const [enableSSL, setEnableSSL] = useState(!!config?.ssl_enable);
  const [caFile, setCaFile] = useState("");
  const [consistency, setConsistency] = useState(config?.consistency || 1);
  function makeConfig(): DBConnectionConfig {
    return {
      conn_type: "cassandra",
      name: connectionName,
      host,
      consistency,
      username: user ? user : "",
      password: password ? password : null,
      db: database ? database : null,
      save_password: savePassword,
      read_only: readOnly,
      conn_timeout: connTimeOut,
      port: parseInt(port),
      dialect,
      ssl_enable: enableSSL,
      ssl_ca_file: caFile ? caFile : null,
    };
  }
  makeConfigRef.current = makeConfig;
  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <SSLNotice backend="openssl" />
      <Stack direction="row" style={{ alignItems: "center" }}>
        <FormControlLabel
          style={{ width: "180px" }}
          control={
            <Checkbox
              checked={enableSSL}
              onChange={(e) => setEnableSSL(e.target.checked)}
            />
          }
          label="Enable SSL"
        />
        {enableSSL && (
          <SelectPath
            path={caFile}
            setPath={setCaFile}
            required={false}
            label="ca file "
          />
        )}
      </Stack>
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        label="key space"
        makeConfig={makeConfig}
      />
      <Stack direction="row" style={{ alignItems: "center" }}>
        <FormControlLabel
          control={
            <Slider
              style={{ width: "9rem" }}
              step={1}
              min={0}
              max={9}
              value={consistency}
              onChange={(_, value) => {
                setConsistency(value);
              }}
            />
          }
          label="Consistency"
        />
        <UrlDocument url="https://docs.rs/scylla/latest/scylla/frame/types/enum.Consistency.html" />
      </Stack>
    </Stack>
  );
}

function SqlServer(props: {
  config?: SqlServerDBConfig;
  context: AddConnectionContext;
}) {
  const { config, context } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || 1433) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [savePassword, setSavePassword] = useState(getSavePassword(config));

  const [database, setDatabase] = useState(getDB(config));
  const [sqlserverCert, setSqlServerCert] = useState<SqlServerCertData>({
    trust_cert: config ? config.trust_cert : false,
    cert_ca: config ? config.cert_ca : "",
  });
  const [schema, setSchema] = useState(getSchema(config));
  const { connTimeOut, connectionName, readOnly, makeConfigRef } = context;

  function makeConfig(): DBConnectionConfig {
    return {
      conn_type: "sqlserver",
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
      cert_ca: sqlserverCert.cert_ca ? sqlserverCert.cert_ca : null,
      schema: schema ? schema : null,
    };
  }
  makeConfigRef.current = makeConfig;

  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        label="database"
        makeConfig={makeConfig}
      />
      {database && (
        <DbSchemaCadidates
          name={schema}
          setName={setSchema}
          label="schema"
          makeConfig={makeConfig}
        />
      )}
      <SqlServerCert data={sqlserverCert} setData={setSqlServerCert} />
    </Stack>
  );
}

function AddConnection(props: { config?: DBConnectionConfig }) {
  const { config } = props;
  const [connectionType, setConnectionType] = useState<DataSourceKind | "scylla">(
    (config?.conn_type as DataSourceKind) || DataSourceKind.Mysql,
  );
  const [connectionName, setConnectionName] = useState(config?.name || "");
  const [connTimeout, setConnTimeout] = useState<string>(
    config && config.conn_timeout ? config.conn_timeout.toString() : "",
  );
  const connectionNameDom = useRef<HTMLInputElement>(null);
  const [readOnly, setReadOnly] = useState(config?.read_only);
  const [cnameDuplicated, setCnameDuplicated] = useState(false);

  const nameExits = useConnectionStore((s) => s.nameExists);

  useEffect(() => {
    setCnameDuplicated(nameExits(connectionName));
  }, [connectionName, nameExits]);

  const makeConfigRef = useRef<MakeConfig | null>(null);

  const context = useMemo(() => {
    return {
      connectionName,
      connectionNameDom,
      readOnly: !!readOnly,
      connTimeOut: connTimeout ? parseInt(connTimeout) : null,
      makeConfigRef,
    };
  }, [connTimeout, connectionName, readOnly]);

  return (
    <Stack style={{ maxWidth: "800px", margin: "0 auto" }}>
      <FormControl fullWidth>
        <InputLabel>Connection Type</InputLabel>
        <Select
          value={connectionType}
          label="Connection Type"
          onChange={(e) => setConnectionType(e.target.value as DataSourceKind)}
          disabled={!!config}>
          <MenuItem value={DataSourceKind.Mysql}>MySQL</MenuItem>
          <MenuItem value={DataSourceKind.MariaDB}>MariaDB</MenuItem>
          <MenuItem value={DataSourceKind.TiDB}>TIDB</MenuItem>
          <MenuItem value={DataSourceKind.Postgres}>PostgreSQL</MenuItem>
          <MenuItem value={DataSourceKind.Sqlite}>SQLite</MenuItem>
          <MenuItem value={DataSourceKind.Redis}>Redis</MenuItem>
          <MenuItem value={DataSourceKind.Mongo}>Mongo</MenuItem>
          <MenuItem value={DataSourceKind.Oracle}>Oracle</MenuItem>
          <MenuItem value={DataSourceKind.SqlServer}>Sql Server(microsoft)</MenuItem>
          <MenuItem value={DataSourceKind.Cassandra}>Cassandra</MenuItem>
          <MenuItem value="scylla">Scylla</MenuItem>
          <MenuItem value={DataSourceKind.ClickHouse}>Clickhouse</MenuItem>
          <MenuItem value={DataSourceKind.LibSql}> LibSql </MenuItem>
          <MenuItem value={DataSourceKind.FireBird}> FireBird </MenuItem>
          <MenuItem value={DataSourceKind.DuckDB}> Duckdb </MenuItem>
          <MenuItem value={DataSourceKind.Cockroach}> Cockroach </MenuItem>

          <MenuItem value={DataSourceKind.CouchBase}> CouchBase </MenuItem>
          {/* <MenuItem value={DataSourceKind.Db2}> Db2(luw) </MenuItem> */}
        </Select>
      </FormControl>
      <Stack direction="row" style={{ alignItems: "flex-start" }}>
        <ClearableTextField
          label="Connection Name"
          value={connectionName}
          inputRef={connectionNameDom}
          required
          onChange={(e) => setConnectionName(e.target.value)}
          onClear={() => setConnectionName("")}
          error={cnameDuplicated}
          helperText={cnameDuplicated ? "Connection name duplicated" : ""}
          style={{ flex: 1 }}
        />
        <ClearableTextField
          label="connection timeout"
          value={connTimeout}
          style={{ flex: 1 }}
          onChange={(e) => setConnTimeout(e.target.value)}
          onClear={() => setConnTimeout("")}
          error={connTimeout ? isNaN(parseInt(connTimeout)) : false}
          helperText="milliseconds to wait for connect"
        />
        <FormControlLabel
          style={{ width: "180px" }}
          control={
            <Checkbox
              checked={readOnly}
              onChange={(e) => setReadOnly(e.target.checked)}
            />
          }
          label="Read Only"
        />
      </Stack>

      <Stack>
        <Typography variant="h6"> server info </Typography>
        <Divider />
        {(connectionType === DataSourceKind.Mysql ||
          connectionType === DataSourceKind.MariaDB) && (
          <Mysql
            context={context}
            config={config as MysqlDBConfig}
            kind={connectionType as DataSourceKind}
          />
        )}

        {connectionType === DataSourceKind.TiDB && (
          <Mysql
            context={context}
            config={config as MysqlDBConfig}
            kind={connectionType as DataSourceKind}
          />
        )}

        {connectionType === DataSourceKind.LibSql && (
          <LibSql context={context} config={config as LibSqlDBConfig} />
        )}

        {connectionType === DataSourceKind.FireBird && (
          <FireBird context={context} config={config as FireBirdConfig} />
        )}

        {(connectionType === DataSourceKind.Postgres ||
          connectionType === DataSourceKind.Cockroach) && (
          <PostGres
            context={context}
            config={config as PGDBConfig}
            kind={connectionType as DataSourceKind}
          />
        )}
        {connectionType === DataSourceKind.ClickHouse && (
          <ClickHouse context={context} config={config as ClickHouseConfig} />
        )}

        {connectionType === DataSourceKind.Cassandra && (
          <Cassandra
            context={context}
            config={config as CassandraDBConfig}
            dialect={"Cassandra"}
          />
        )}
        {connectionType == "scylla" && (
          <Cassandra
            context={context}
            config={config as CassandraDBConfig}
            dialect="Scylla"
          />
        )}
        {connectionType === DataSourceKind.Redis && (
          <Redis context={context} config={config as RedisDBConfig} />
        )}
        {connectionType === DataSourceKind.Mongo && (
          <MongoDb context={context} config={config as MongoDBConfig} />
        )}
        {connectionType === DataSourceKind.SqlServer && (
          <SqlServer context={context} config={config as SqlServerDBConfig} />
        )}
        {connectionType === DataSourceKind.Oracle && (
          <Oracle context={context} config={config as OracleDBConfig} />
        )}
        {connectionType === DataSourceKind.Sqlite && (
          <Sqlite context={context} config={config as SqliteDBConfig} />
        )}
        {connectionType === DataSourceKind.DuckDB && (
          <DuckDB context={context} config={config as DuckDBConfig} />
        )}

        {connectionType === DataSourceKind.Db2 && (
          <Db2 context={context} config={config as Db2Config} />
        )}

        {connectionType === DataSourceKind.CouchBase && (
          <CouchBase context={context} config={config as CouchBaseDBConfig} />
        )}
      </Stack>

      <TestConnAndSave
        connectionNameDom={connectionNameDom}
        makeConfig={() => makeConfigRef.current!()}
        cnameDuplicated={cnameDuplicated}
      />
    </Stack>
  );
}

function DuckDB(props: { config?: DuckDBConfig; context: AddConnectionContext }) {
  const { config, context } = props;

  function getConfigStr(key: string) {
    if (!config) {
      return "";
    }
    const v = config[key];
    return v ? v : "";
  }
  const [path, setPath] = useState(getConfigStr("path"));
  const [db, setDb] = useState(getConfigStr("db"));
  const [schema, setSchema] = useState(getConfigStr("schema"));
  const { connTimeOut, connectionName, readOnly, makeConfigRef } = context;
  const [maxMemory, setMaxMemory] = useState("");
  function makeConfig(): DBConnectionConfig {
    const x: DBConnectionConfig = {
      conn_type: "duckdb",
      path,
      db: db ? db : null,
      conn_timeout: connTimeOut,
      name: connectionName,
      read_only: readOnly,
      max_memory: maxMemory ? maxMemory : null,
      schema: schema ? schema : null,
    };
    console.info("makeConfig", x);
    return x;
  }
  makeConfigRef.current = makeConfig;
  return (
    <Stack>
      <SelectPath path={path} setPath={setPath} label="database file path" required />
      {path && (
        <DbSchemaCadidates
          name={db}
          setName={setDb}
          makeConfig={makeConfig}
          label="database"
        />
      )}
      {db && (
        <DbSchemaCadidates
          name={schema}
          setName={setSchema}
          label="schema"
          makeConfig={makeConfig}
        />
      )}

      <TextField
        label="max memory"
        value={maxMemory}
        onChange={(e) => {
          setMaxMemory(e.target.value);
        }}
      />
    </Stack>
  );
}

function LibSql(props: { config?: LibSqlDBConfig; context: AddConnectionContext }) {
  const { config, context } = props;
  const urlOld = config && config.source == "remote" && config.url;
  const [url, setUrl] = useState(urlOld ? urlOld : "");
  // const [replica, setReplica] = useState(config?.replica);
  const oldAuth = config && config.source == "remote" && config.auth_token;
  const [authToken, setAuthToken] = useState(oldAuth ? oldAuth : "");
  const [libSqlModel] = useState<"remote" | "local">(config?.source || "remote");
  const [path] = useState(config?.path);
  const { connTimeOut, connectionName, readOnly, makeConfigRef } = context;

  function makeConfig(): DBConnectionConfig {
    const x: DBConnectionConfig = {
      conn_type: "libsql",
      name: connectionName,
      conn_timeout: connTimeOut,
      source: libSqlModel,
      url: url,
      auth_token: authToken,
      replica: false,
      read_only: readOnly,
      path: path ? path : null,
    };
    console.info("makeConfig", x);
    return x;
  }

  makeConfigRef.current = makeConfig;

  // const models = ['remote', 'local'];
  return (
    <>
      {/* <RadioGroup
        row
        onChange={(e) => {
          setLibSqlModal(e.target.value as 'remote');
        }}
        value={libSqlModel}
      >
        {models.map((e) => (
          <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
        ))}
      </RadioGroup> */}

      {/* {libSqlModel === "local" && (
        <SelectPath
          path={path}
          setPath={setPath}
          required
          label="libsql database file"
        />
      )} */}
      {libSqlModel === "remote" && (
        <>
          <Stack direction="row">
            <ClearableTextField
              value={url}
              label="url"
              onChange={(e) => {
                setUrl(e.target.value);
              }}
              onClear={() => setUrl("")}
              fullWidth
            />
            <ClearableTextField
              fullWidth
              value={authToken}
              label="auth token "
              onChange={(e) => {
                setAuthToken(e.target.value);
              }}
              onClear={() => setAuthToken("")}
            />
          </Stack>
          {/* <Stack
            direction="row"
            style={{ alignItems: 'center' }}
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
          </Stack> */}
        </>
      )}
    </>
  );
}

function OdbcDriverCommon(props: { config: RefObject<OdbcCommon> }) {
  const { config } = props;
  const [driverName, setDriverName] = useState(config.current.driver_name);
  const [maxStrLen, setMaxStrLen] = useState(config.current.max_str_len + "");
  const [maxbinLen, setMaxBinLen] = useState(config.current.max_bin_len + "");

  useEffect(() => {
    // eslint-disable-next-line react-hooks/immutability
    config.current.driver_name = driverName;
    config.current.max_str_len = parseInt(maxStrLen);
    config.current.max_bin_len = parseInt(maxbinLen);
  }, [config, driverName, maxStrLen, maxbinLen]);

  return (
    <Stack direction="row">
      <ClearableTextField
        label="driver name"
        value={driverName}
        sx={{ mr: 1 }}
        onChange={(e) => setDriverName(e.target.value)}
        onClear={() => setDriverName("")}
        required
      />

      <ClearableTextField
        label="max string length"
        value={maxStrLen}
        sx={{ mr: 1 }}
        onChange={(e) => setMaxStrLen(e.target.value)}
        onClear={() => setMaxStrLen("")}
        error={maxStrLen ? isNaN(parseInt(maxStrLen)) : false}
        required
      />

      <ClearableTextField
        label="max bin length"
        value={maxbinLen}
        sx={{ mr: 1 }}
        onChange={(e) => setMaxBinLen(e.target.value)}
        onClear={() => setMaxBinLen("")}
        error={maxbinLen ? isNaN(parseInt(maxbinLen)) : false}
        required
      />
    </Stack>
  );
}

function Db2(props: { config?: Db2Config; context: AddConnectionContext }) {
  const { config, context } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState<string>((config?.port || 50000) + "");

  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [database, setDatabase] = useState(getDB(config));
  const [schema, setSchema] = useState(getSchema(config));
  const { connTimeOut, connectionName, readOnly, makeConfigRef } = context;

  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const odbccommon = useRef<OdbcCommon>({
    driver_name: "Driver_IBM_DB2",
    max_str_len: 4096,
    max_bin_len: 4096,
  });

  function makeConfig(): DBConnectionConfig {
    return {
      conn_type: "db2",
      host,
      port: parseInt(port),
      username: user,
      password: password,
      save_password: savePassword,
      db: database,
      schema: schema ? schema : null,
      conn_timeout: connTimeOut,
      read_only: readOnly,
      name: connectionName,
      ...odbccommon.current,
    };
  }
  makeConfigRef.current = makeConfig;
  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <ClearableTextField
        label="database"
        value={database}
        sx={{ mr: 1 }}
        onChange={(e) => setDatabase(e.target.value)}
        onClear={() => setDatabase("")}
        required
      />
      {database && (
        <DbSchemaCadidates
          name={schema}
          setName={setSchema}
          label="schema"
          makeConfig={makeConfig}
        />
      )}
      <OdbcDriverCommon config={odbccommon} />
    </Stack>
  );
}

function AccInvalidHostNameAndCerts({
  accInvalidCerts,
  setAccInvalidCerts,
  accInvalidHosts,
  setAccInvalidHosts,
}) {
  return (
    <>
      <FormControlLabel
        control={
          <Checkbox
            checked={accInvalidCerts}
            onChange={(event) => setAccInvalidCerts(event.target.checked)}
          />
        }
        label="accpet invalid certs"
      />
      <FormControlLabel
        control={
          <Checkbox
            checked={accInvalidHosts}
            onChange={(event) => setAccInvalidHosts(event.target.checked)}
          />
        }
        label="accpet invalid host names"
      />
    </>
  );
}

function CouchBase(props: {
  config?: CouchBaseDBConfig;
  context: AddConnectionContext;
}) {
  const { config, context } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState(
    (function () {
      if (!config) {
        return "11210";
      }
      return config.port + "";
    })(),
  );
  const [user, setUser] = useState(getUser(config) || "Administrator");
  const [password, setPassword] = useState(getPassword(config));
  const [bucket, setBucket] = useState(getDB(config));
  const [scope, setScope] = useState(getSchema(config));
  const { connTimeOut, connectionName, readOnly, makeConfigRef } = context;
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const [sslEnable, setSslEnable] = useState(false);
  const [accInvalidHosts, setAccInvalidHosts] = useState(false);
  const [accInvalidCerts, setAccInvalidCerts] = useState(false);
  const [certFile, setCertFile] = useState("");

  function makeConfig(): DBConnectionConfig {
    return {
      conn_type: "couchbase",
      host: host,
      port: parseInt(port),
      username: user,
      password: password,
      save_password: savePassword,
      bucket: bucket ? bucket : null,
      scope: scope ? scope : null,
      conn_timeout: connTimeOut,
      read_only: readOnly,
      name: connectionName,
      ssl_enable: sslEnable,
      danger_accept_invalid_certificates: accInvalidCerts,
      danger_accept_invalid_hostnames: accInvalidHosts,
      ca_certificates: certFile ? [certFile] : [],
    };
  }

  useEffect(() => {
    if (sslEnable) {
      setPort("11207");
    } else {
      setPort("11210");
    }
  }, [sslEnable]);

  makeConfigRef.current = makeConfig;
  return (
    <Stack>
      <HostAndPort host={host} setHost={setHost} port={port} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <DbSchemaCadidates
        name={bucket}
        setName={setBucket}
        label="bucket"
        makeConfig={makeConfig}
      />
      <Stack direction="row" style={{ alignItems: "center" }}>
        <FormControlLabel
          control={
            <Checkbox
              checked={sslEnable}
              onChange={(event) => setSslEnable(event.target.checked)}
            />
          }
          label="SSL"
        />
        {sslEnable && (
          <AccInvalidHostNameAndCerts
            accInvalidCerts={accInvalidCerts}
            setAccInvalidHosts={setAccInvalidHosts}
            accInvalidHosts={accInvalidHosts}
            setAccInvalidCerts={setAccInvalidCerts}
          />
        )}
      </Stack>
      {sslEnable && (
        <Stack direction="row" style={{ alignItems: "center" }}>
          <SelectPath
            path={certFile}
            setPath={setCertFile}
            label="certificate file"
            required={false}
          />
        </Stack>
      )}
      {bucket && (
        <DbSchemaCadidates
          name={scope}
          setName={setScope}
          label="scope"
          makeConfig={makeConfig}
        />
      )}
    </Stack>
  );
}

function SSLNotice(props: { backend: "native-tls" | "openssl" }) {
  return (
    <Alert severity="info">
      SSL connections are implemented using {props.backend} as the backend
    </Alert>
  );
}

export { AddConnection };
