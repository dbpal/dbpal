import {
  Button,
  Checkbox,
  Chip,
  Divider,
  FormControlLabel,
  Stack,
  TextField,
  Tooltip,
} from '@mui/material';
import { useContext, useEffect, useState } from 'react';
import types from './ApiStruct';
import { NotificationContext } from './Notification';
import { save } from '@tauri-apps/plugin-dialog';
import ArrowCircleDownIcon from '@mui/icons-material/ArrowCircleDown';
import { invoke } from '@tauri-apps/api/core';
import { DataSourceKind } from './helper';
import { ModalHeader } from './CommonComp';
import { openPath } from '@tauri-apps/plugin-opener';

function generateDefaultFilename(name, extension) {
  if (extension == 'jsonline') {
    /*
      统一用 .jsonl（全小写，无空格）最好。
      这是社区事实标准：GitHub、Git LFS、Apache Spark、BigQuery、Hugging Face 等生态都按 .jsonl 识别，工具链（grep、jq、ripgrep、VS Code 插件）也都默认支持。
    */
    extension = 'jsonl';
  }
  // 获取当前时间
  const now = new Date();
  // 格式化时间为字符串
  const timestamp = now.toISOString().replace(/[:.-]/g, '').slice(0, 15);
  // 生成文件名
  const filename = `${name}_${timestamp}.${extension}`;
  return filename;
}

function ExportView(props: {
  name: string;
  exportOP: (kind: types.ExportKind, file: string) => Promise<void>;
  supportKind: string[];

  supportCsvHeader: boolean;
}) {
  const { exportOP, supportKind, supportCsvHeader, name } = props;
  const [kind, setKind] = useState(supportKind[0]);
  const [includeCsvHeader, setIncluCsvdeHeader] = useState(false);
  const [delimiter, setDelimiter] = useState(',');
  const [pretty, setPretty] = useState(true);
  const [includeTableSchema, setIncludeTableScheam] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);
  const [file, setFile] = useState<string | null>(null);
  const makeConfig = (): types.ExportKind | null => {
    switch (kind) {
      case 'csv':
        if (delimiter.length > 1) {
          showNotification('error', 'delimiter expect one character');
          return;
        }
        return {
          export_kind: 'csv',
          include_header: includeCsvHeader,
          delimiter,
        };
      case 'sql':
        return { export_kind: 'sql', include_table_schema: includeTableSchema };
      case 'json':
        return { export_kind: 'json', pretty };
      case 'jsonline':
        return { export_kind: 'jsonline' };
    }
  };

  const handleExport = async () => {
    const config = makeConfig();
    if (!config) {
      return;
    }
    const file = await save({
      defaultPath: generateDefaultFilename(name, kind),
    });
    if (!file) {
      return;
    }
    try {
      setLoading(true);
      await exportOP(config, file);
    } catch (e) {
      setLoading(false);
      showNotification('error', 'Export Failure', e);
      return;
    }
    setLoading(false);
    showNotification('success', 'Export OK');
    setFile(file);
  };

  useEffect(() => {
    setFile('');
  }, [kind]);
  return (
    <Stack spacing={1}>
      <ModalHeader title={`export ${name}`}></ModalHeader>
      <Stack direction="row" spacing={1}>
        {supportKind.map((e) => (
          <Chip
            onClick={() => {
              setKind(e);
            }}
            key={e}
            label={e}
            color={e == kind ? 'primary' : 'defaut'}
          ></Chip>
        ))}
      </Stack>
      {kind == 'csv' && (
        <Stack spacing={1}>
          {supportCsvHeader && (
            <FormControlLabel
              control={
                <Checkbox
                  checked={includeCsvHeader}
                  onChange={(e) => {
                    setIncluCsvdeHeader(e.target.checked);
                  }}
                  name="myCheckbox"
                  color="primary"
                />
              }
              label="Include Header"
            />
          )}
          <Stack direction="row">
            <TextField
              size="small"
              value={delimiter}
              onChange={(e) => {
                setDelimiter(e.target.value);
              }}
              label="delimiter"
              error={delimiter.length > 1}
              helperText="only accept one character"
            ></TextField>
          </Stack>
        </Stack>
      )}
      {kind == 'sql' && (
        <Stack>
          <FormControlLabel
            control={
              <Checkbox
                checked={includeTableSchema}
                onChange={(e) => {
                  setIncludeTableScheam(e.target.checked);
                }}
                name="myCheckbox"
                color="primary"
              />
            }
            label="Include Table Schema(if applicable)"
          />
        </Stack>
      )}
      {kind == 'json' && (
        <Stack>
          <FormControlLabel
            control={
              <Checkbox
                checked={pretty}
                onChange={(e) => {
                  setPretty(e.target.checked);
                }}
                name="pretty"
                color="primary"
              />
            }
            label="pretty"
          />
        </Stack>
      )}
      {/* json line have no setttings */}

      {kind == 'jsonline' && <></>}

      <Divider />
      {file && (
        <Button
          onClick={() => {
            openPath(file)
              .then((e) => {
                console.log('open ok', e);
              })
              .catch((e) => {
                showNotification('error', 'open export file failed', e);
              });
          }}
        >
          open with default app (if applicable)
        </Button>
      )}
      <Stack>
        <Button
          onClick={async () => {
            handleExport();
          }}
          loading={loading}
        >
          <Tooltip title="export" arrow>
            <ArrowCircleDownIcon> </ArrowCircleDownIcon>
          </Tooltip>
        </Button>
      </Stack>
    </Stack>
  );
}

function ExportTableOrView(props: {
  uuid: string;
  name: string;
  supportKind: string[];
  kind: DataSourceKind;
}) {
  let { uuid, name, supportKind, kind } = props;
  const supportCsvSet = new Set([
    DataSourceKind.LibSql,
    DataSourceKind.Sqlite,
    DataSourceKind.Postgres,
    DataSourceKind.Cockroach,
    DataSourceKind.Mysql,
    DataSourceKind.MariaDB,
    DataSourceKind.TiDB,
    DataSourceKind.MsSql,
    DataSourceKind.Cassandra,
    DataSourceKind.FireBird,
    DataSourceKind.ClickHouse,
    DataSourceKind.Oracle,
  ]);
  return (
    <ExportView
      exportOP={async (kind, filepath) => {
        return await invoke('export_table_or_view', {
          name: uuid,
          tableOrView: name,
          filepath,
          kind,
        });
      }}
      supportKind={supportKind}
      supportCsvHeader={supportCsvSet.has(kind)}
      name={name}
    ></ExportView>
  );
}

export { ExportView, ExportTableOrView };
