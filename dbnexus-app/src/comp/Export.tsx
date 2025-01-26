import {
  Button,
  Checkbox,
  CircularProgress,
  Divider,
  FormControlLabel,
  Stack,
  TextField,
  Tooltip,
} from '@mui/material';
import { useContext, useState } from 'react';
import types from './ApiStruct';
import { NotificationContext } from './Notification';
import { progressBarSize } from './Common';
import AttachFileIcon from '@mui/icons-material/AttachFile';
import { save } from '@tauri-apps/plugin-dialog';
import ArrowCircleDownIcon from '@mui/icons-material/ArrowCircleDown';
import { invoke } from '@tauri-apps/api/core';
import { DataSourceKind } from './helper';
function ExportView(props: {
  exportOP: (kind: types.ExportKind, file: string) => Promise<void>;
  support_kind: string[];
  handleClose;
  supportCsvHeader: boolean;
}) {
  const { exportOP, handleClose, support_kind, supportCsvHeader } = props;
  const [kind, setKind] = useState('csv');
  const [includeCsvHeader, setIncluCsvdeHeader] = useState(false);
  const [delimiter, setDelimiter] = useState(',');
  const [pretty, setPretty] = useState(true);
  const [includeTableSchema, setIncludeTableScheam] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);
  const [file, setFile] = useState('');
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
    setLoading(true);
    try {
      await exportOP(config, file);
    } catch (e) {
      setLoading(false);
      showNotification('error', 'Export Failure', e);
      return;
    }
    setLoading(false);

    showNotification('success', 'Export OK');
    handleClose();
  };

  return (
    <Stack spacing={1}>
      {loading && <CircularProgress size={progressBarSize} />}
      <Stack direction="row" spacing={1}>
        {support_kind.map((e) => (
          <Button
            disabled={kind == e}
            onClick={() => {
              setKind(e);
            }}
            key={e}
            variant="contained"
          >
            {e}
          </Button>
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
      <Stack direction="row" sx={{ alignItems: 'flex-end' }}>
        <Button
          onClick={async () => {
            let file = await save({ title: 'save file' });
            setFile(file);
          }}
        >
          <AttachFileIcon></AttachFileIcon>
        </Button>
        <span> file:{file} </span>
      </Stack>
      <Divider></Divider>
      <Stack>
        <Button
          disabled={!file}
          onClick={async () => {
            handleExport();
          }}
        >
          <Tooltip title="export">
            <ArrowCircleDownIcon> </ArrowCircleDownIcon>
          </Tooltip>
        </Button>
      </Stack>
    </Stack>
  );
}

function ExportTableOrView(props: {
  handleClose;
  uuid: string;
  name: string;
  support_kind: string[];
  kind: DataSourceKind;
}) {
  let { handleClose, uuid, name, support_kind, kind } = props;

  return (
    <ExportView
      handleClose={handleClose}
      exportOP={async (kind, filepath) => {
        return await invoke('export_table_or_view', {
          name: uuid,
          tableOrView: name,
          filepath,
          kind,
        });
      }}
      support_kind={support_kind}
      supportCsvHeader={
        kind == DataSourceKind.Sqlite ||
        kind == DataSourceKind.Postgres ||
        kind == DataSourceKind.Mysql ||
        kind == DataSourceKind.MariaDB ||
        kind == DataSourceKind.MsSql
      }
    ></ExportView>
  );
}

export { ExportView, ExportTableOrView };
