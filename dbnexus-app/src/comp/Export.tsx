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
function ExportView(props: {
  exportOP: (kind: types.ExportKind, file: string) => Promise<void>;
  handleClose;
}) {
  const { exportOP, handleClose } = props;
  const [kind, setKind] = useState('csv');
  const exportKinds = ['csv', 'sql', 'json', 'jsonline'];
  const [includeHeader, setIncludeHeader] = useState(false);
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
        return { export_kind: 'csv', include_header: includeHeader, delimiter };
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
      <Stack direction='row' spacing={1}>
        {exportKinds.map((e) => (
          <Button
            disabled={kind == e}
            onClick={() => {
              setKind(e);
            }}
            variant='contained'
          >
            {e}
          </Button>
        ))}
      </Stack>
      {kind == 'csv' && (
        <Stack spacing={1}>
          <FormControlLabel
            control={
              <Checkbox
                checked={includeHeader}
                onChange={(e) => {
                  setIncludeHeader(e.target.checked);
                }}
                name='myCheckbox'
                color='primary'
              />
            }
            label='Include Header'
          />
          <Stack direction='row'>
            <TextField
              value={delimiter}
              onChange={(e) => {
                setDelimiter(e.target.value);
              }}
              size='small'
              label='delimiter'
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
                name='myCheckbox'
                color='primary'
              />
            }
            label='Include Table Schema(if applicable)'
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
                name='pretty'
                color='primary'
              />
            }
            label='pretty'
          />
        </Stack>
      )}
      {/* json line have no setttings */}

      {kind == 'jsonline' && <></>}
      <Stack direction='row' spacing={1} alignContent='center'>
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
          <Tooltip title='export'>
            <ArrowCircleDownIcon> </ArrowCircleDownIcon>
          </Tooltip>
        </Button>
      </Stack>
    </Stack>
  );
}

function ExportTableView(props: { handleClose; uuid: string; table: string }) {
  let { handleClose, uuid, table } = props;

  return (
    <ExportView
      handleClose={handleClose}
      exportOP={async (kind, filepath) => {
        return await invoke('export_table', {
          name: uuid,
          tableName: table,
          filepath,
          kind,
        });
      }}
    ></ExportView>
  );
}

export { ExportView, ExportTableView };
