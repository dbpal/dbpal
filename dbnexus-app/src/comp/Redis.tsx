import {
  CSSProperties,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import {
  Button,
  Stack,
  Box,
  TextField,
  MenuItem,
  FormControl,
  InputLabel,
  Select,
  Typography,
  Modal,
  Paper,
  MenuList,
  ListItemIcon,
  ListItemText,
  RadioGroup,
  Radio,
  FormControlLabel,
  Chip,
} from '@mui/material';
import { v4 as uuidv4 } from 'uuid';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import {
  modalStyle,
  SettingContext,
  reloadOK,
  SimpleResizeEditor,
  EscDownEvent,
} from './Common.tsx';

import { types } from './ApiStruct.ts';
import { openUrl } from '@tauri-apps/plugin-opener';
import { NotificationContext } from './Notification';
import {
  DataGrid,
  GridPaginationModel,
  GridSlotsComponentsProps,
  GridSortModel,
} from '@mui/x-data-grid';

import {
  calMenuStyle,
  ContextMenuContext,
  handleCopyToClipboard,
  hexToString,
  hexTryToString,
  stringToHex,
  VisiableContext,
} from './helper.ts';
import { pageSizeOptions } from './CommonComp.tsx';

class RedisZsetHashModifyReq {
  constructor(
    public oldKey: string,
    public newKey: string,
    public value: string
  ) {}
}

enum RedisEditorMode {
  Hex,
  Text,
  Json,
}

function RedisAddList(props: {
  lpushOP: (value: string) => Promise<void>;
  rpushOP: (value: string) => Promise<void>;
}) {
  const { lpushOP, rpushOP } = props;
  const [loading, setLoading] = useState(false);
  const [direction, setDirection] = useState<'left' | 'right'>('left');
  const [key, setKey] = useState('');
  const { showNotification } = useContext(NotificationContext);
  const handleAdd = () => {
    // reset some state
    setLoading(true);
    (direction == 'left' ? lpushOP : rpushOP)(key)
      .then(() => {
        showNotification('success', 'Push OK');
      })
      .catch((e) => {
        showNotification('error', 'Push OK', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  return (
    <Stack spacing={1}>
      <Stack direction="row" style={{ alignItems: 'center' }} spacing={1}>
        <Typography color="info"> direction: </Typography>
        <RadioGroup
          row
          onChange={(_e, value) => {
            //@ts-ignore
            setDirection(value);
          }}
          value={direction}
        >
          <FormControlLabel value="left" control={<Radio />} label="head" />
          <FormControlLabel value="right" control={<Radio />} label="tail" />
        </RadioGroup>
      </Stack>

      <RedisValueEditor
        handleValue={(e) => setKey(e)}
        editorHeight="60vh"
      ></RedisValueEditor>

      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          onClick={handleAdd}
        >
          push
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisStringTable(props: {
  modifyOP: (content: string) => Promise<void>;
  content: string;
}) {
  const { modifyOP, content } = props;
  const [newContent, setNewConent] = useState(content);
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const handleModify = () => {
    setLoading(true);
    modifyOP(newContent)
      .then(() => {
        showNotification('success', 'Modify OK');
      })
      .catch((e) => {
        showNotification('error', 'Modify failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <RedisValueEditor
        oldValue={newContent}
        handleValue={(e) => {
          setNewConent(e);
        }}
        editorHeight="60vh"
      ></RedisValueEditor>

      <Stack width="100px" direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          onClick={() => {
            handleModify();
          }}
        >
          set
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisAddSet(props: { addOP: (key: string) => Promise<void> }) {
  let { addOP } = props;
  const [loading, setLoading] = useState(false);
  const [key, setKey] = useState('');
  const { showNotification } = useContext(NotificationContext);
  const handleAdd = () => {
    // reset some state
    setLoading(true);
    addOP(key)
      .then(() => {
        showNotification('success', 'Add ok');
      })
      .catch((e) => {
        showNotification('error', 'Add error', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <RedisValueEditor
        handleValue={(e) => {
          setKey(e);
        }}
        editorHeight="60vh"
      ></RedisValueEditor>

      <Stack direction="row" spacing={1}>
        <Button
          color="primary"
          variant="contained"
          loading={loading}
          onClick={handleAdd}
        >
          add
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisAddHash(props: {
  addOP: (key: string, value: string) => Promise<void>;
}) {
  let { addOP } = props;
  const [loading, setLoading] = useState(false);
  const [value, setValue] = useState('');
  const [key, setKey] = useState('');

  const { showNotification } = useContext(NotificationContext);
  const handleAdd = () => {
    // reset some state
    setLoading(true);
    addOP(key, value)
      .then(() => {
        showNotification('success', 'Add OK');
      })
      .catch((e) => {
        showNotification('success', 'Add Error', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <RedisValueEditor
        oldValue={key}
        handleValue={(e) => {
          setKey(e);
        }}
        editorHeight="40vh"
        header=<span color="info"> key </span>
      ></RedisValueEditor>

      <RedisValueEditor
        oldValue={value}
        handleValue={(e) => {
          setValue(e);
        }}
        editorHeight="40vh"
        header=<span color="info"> value </span>
      ></RedisValueEditor>

      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          onClick={handleAdd}
        >
          Add
        </Button>
      </Stack>
    </Stack>
  );
}
function RedisAddZset(props: {
  addOP: (key: string, value: number) => Promise<void>;
}) {
  let { addOP } = props;
  const [loading, setLoading] = useState(false);
  const [score, setScore] = useState('');
  const [key, setKey] = useState('');
  const [scoreErr, setScoreErr] = useState(false);

  const { showNotification } = useContext(NotificationContext);

  const handleAdd = () => {
    // reset some state
    setLoading(true);
    addOP(key, parseFloat(score))
      .then(() => {
        showNotification('success', 'Add OK');
      })
      .catch((e) => {
        showNotification('success', 'Add Failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <TextField
        size="small"
        value={score}
        label="score"
        error={scoreErr}
        helperText="put a valid float64 value"
        onChange={(event) => {
          setScore(event.target.value);
          setScoreErr(isNaN(parseFloat(event.target.value)));
        }}
      ></TextField>

      <RedisValueEditor
        handleValue={(e) => {
          setKey(e);
        }}
        editorHeight="60vh"
      ></RedisValueEditor>

      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          onClick={handleAdd}
        >
          Add
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisEditHash(props: {
  modifyOP: (req: RedisZsetHashModifyReq) => Promise<void>;
  oldKey: string;
  oldValue: string;
  handleClose;
}) {
  const { oldKey, oldValue, modifyOP, handleClose } = props;
  const [loading, setLoading] = useState(false);
  const [newKey, setNewKey] = useState(oldKey);
  const [newValue, setNewValue] = useState(oldValue);
  const { showNotification } = useContext(NotificationContext);

  const handleModify = () => {
    // reset some state
    setLoading(true);
    modifyOP(new RedisZsetHashModifyReq(oldKey, newKey, newValue))
      .then(() => {
        showNotification('success', 'Modify OK');

        handleClose();
      })
      .catch((e) => {
        showNotification('success', 'Modify failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <RedisValueEditor
        oldValue={newKey}
        handleValue={(e) => {
          setNewKey(e);
        }}
        editorHeight="29vh"
        header=<span color="info"> key </span>
      ></RedisValueEditor>

      <RedisValueEditor
        oldValue={newValue}
        handleValue={(e) => {
          setNewValue(e);
        }}
        editorHeight="29vh"
        header=<span color="info"> value </span>
      ></RedisValueEditor>

      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          onClick={handleModify}
        >
          modify
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisEditZset(props: {
  modifyOP: (oldkey: string, newkey: string, value: number) => Promise<void>;
  oldKey: string;
  oldValue: string;
  handleClose: () => void;
}) {
  const { oldKey, oldValue, modifyOP, handleClose } = props;
  const [loading, setLoading] = useState(false);

  const [newKey, setNewKey] = useState(oldKey);
  const [newValue, setNewValue] = useState(oldValue);
  const [newValueErr, setNewValueErr] = useState(false);
  const { showNotification } = useContext(NotificationContext);

  const handleModify = () => {
    if (oldKey == newKey && oldValue == newValue) {
      return;
    }
    // reset some state
    setLoading(true);

    modifyOP(oldKey, newKey, parseInt(newValue))
      .then(() => {
        showNotification('success', 'Modify OK');
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Modify Failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <TextField
        size="small"
        value={newValue}
        label="score"
        error={newValueErr}
        helperText="put a valid float64 value"
        onChange={(event) => {
          setNewValue(event.target.value);
          if (isNaN(parseFloat(event.target.value))) {
            setNewValueErr(true);
          } else {
            setNewValueErr(false);
          }
        }}
      ></TextField>
      <RedisValueEditor
        oldValue={oldKey}
        handleValue={(e) => {
          setNewKey(e);
        }}
        editorHeight="60vh"
      ></RedisValueEditor>

      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          onClick={handleModify}
        >
          modify
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisEditModeSelect(props: {
  mode: RedisEditorMode;
  handleChange: (mode: RedisEditorMode) => void;
}) {
  const { mode, handleChange } = props;
  return (
    <FormControl fullWidth>
      <InputLabel id="simple-select-label">viewer</InputLabel>
      <Select
        size="small"
        value={mode}
        onChange={(e) => {
          handleChange(e.target.value as RedisEditorMode);
        }}
        label="viewer"
      >
        <MenuItem value={RedisEditorMode.Text}>text</MenuItem>
        <MenuItem value={RedisEditorMode.Json}>json</MenuItem>
        <MenuItem value={RedisEditorMode.Hex}>hex</MenuItem>
      </Select>
    </FormControl>
  );
}

function RedisValueEditor(props: {
  oldValue?: string;
  handleValue: (a: string) => void;
  editorHeight: string;
  header?: ReactNode;
}) {
  let { oldValue, handleValue, editorHeight, header } = props;
  const [newValue, setNewValue] = useState<string>('');
  const [editModel, setEditModel] = useState<RedisEditorMode>(
    RedisEditorMode.Text
  );
  useEffect(() => {
    let value = hexTryToString(oldValue);
    if (typeof value === 'string') {
      let isJson = false;
      try {
        isJson = typeof JSON.parse(value) == 'object';
      } catch (e) {}
      if (isJson) {
        setEditModel(RedisEditorMode.Json);
      } else {
        setEditModel(RedisEditorMode.Text);
      }
      setNewValue(value);
    } else {
      setEditModel(RedisEditorMode.Hex);
      setNewValue(oldValue);
    }
  }, []);
  useEffect(() => {
    switch (editModel) {
      case RedisEditorMode.Hex:
        handleValue(newValue);
        break;
      default:
        handleValue(stringToHex(newValue));
    }
  }, [newValue]);

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        {header}
        {editModel == RedisEditorMode.Hex && (
          <>
            <span style={{ width: '18rem', textAlign: 'center' }}>
              looking for hex editor???
            </span>
            <Chip
              label="try hexed.it"
              onClick={() => {
                openUrl('https://hexed.it/');
              }}
              size="small"
            ></Chip>
          </>
        )}
        <RedisEditModeSelect
          mode={editModel}
          handleChange={(mode) => {
            if (
              mode == RedisEditorMode.Hex &&
              (editModel == RedisEditorMode.Json ||
                editModel == RedisEditorMode.Text)
            ) {
              setNewValue(stringToHex(newValue));
            }
            if (
              (mode == RedisEditorMode.Json || mode == RedisEditorMode.Text) &&
              editModel == RedisEditorMode.Hex
            )
              setNewValue(hexToString(newValue));
            setEditModel(mode);
          }}
        ></RedisEditModeSelect>
      </Stack>

      <SimpleResizeEditor
        language={editModel == RedisEditorMode.Json ? 'json' : 'text'}
        // in some times newvalue is not change as a string
        value={typeof newValue == 'object' ? '' : newValue}
        onMount={(editor) => {
          editor.onDidChangeModelContent(() => {
            const value = editor.getValue();
            setNewValue(value);
          });
        }}
        wordWrap="on"
        style={{ height: editorHeight }}
      ></SimpleResizeEditor>
    </Stack>
  );
}

function RedisEditList(props: {
  modifyOP: (index: number, newValue: string) => Promise<void>;
  oldValue: string;
  index: number;
  handleClose: () => void;
}) {
  let { oldValue, modifyOP, handleClose, index } = props;
  const [loading, setLoading] = useState(false);
  const [newValue, setNewValue] = useState('');
  const { showNotification } = useContext(NotificationContext);

  // really modify the content.
  const handleModify = () => {
    setLoading(true);
    modifyOP(index, newValue)
      .then(() => {
        setNewValue('');
        showNotification('success', 'Modify OK', '');
        handleClose();
      })
      .catch((e) => {
        showNotification('success', 'Modify Failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <RedisValueEditor
        oldValue={oldValue}
        handleValue={(e) => {
          setNewValue(e);
        }}
        editorHeight="60vh"
      ></RedisValueEditor>
      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          disabled={newValue == ''}
          onClick={handleModify}
        >
          modify
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisEditSet(props: {
  modifyOP: (oldValue: string, newValue: string) => Promise<void>;
  oldValue: string;
  handleClose: () => void;
}) {
  let { oldValue, modifyOP, handleClose } = props;
  const [loading, setLoading] = useState(false);
  const [newValue, setNewValue] = useState('');
  const { showNotification } = useContext(NotificationContext);

  // really modify the content.
  const handleModify = () => {
    setLoading(true);
    modifyOP(oldValue, newValue)
      .then(() => {
        setNewValue('');
        showNotification('success', 'Modify OK');
        handleClose();
      })
      .catch((e) => {
        showNotification('success', 'Modify Failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <RedisValueEditor
        oldValue={oldValue}
        handleValue={(e) => {
          setNewValue(e);
        }}
        editorHeight="60vh"
      ></RedisValueEditor>
      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          disabled={newValue == ''}
          onClick={handleModify}
        >
          modify
        </Button>
      </Stack>
    </Stack>
  );
}

function redisRenderCell(p: any) {
  let value = hexTryToString(p);
  if (typeof value === 'string') {
    return value;
  } else {
    return 'BINARY HEX ' + p;
  }
}

function RedisSetTableContextMenu(props: { delOP: () => void; content }) {
  const { delOP, content } = props;
  const { showNotification } = useContext(NotificationContext);
  return (
    <Paper>
      <MenuList>
        {content && (
          <MenuItem
            onClick={() => {
              handleCopyToClipboard(content, showNotification);
            }}
          >
            <ListItemIcon>
              <ContentCopyIcon></ContentCopyIcon>
            </ListItemIcon>
            <ListItemText> Copy </ListItemText>
          </MenuItem>
        )}

        <MenuItem
          onClick={() => {
            delOP();
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText> Delete </ListItemText>
        </MenuItem>
      </MenuList>
    </Paper>
  );
}

function RedisSetTable(props: {
  reloadOP: (keyMatch: string) => Promise<types.MuiDataGridData>;
  modifyOP: (oldvalue: string, newValue: string) => Promise<void>;

  delOP: (value: string) => Promise<void>;
}) {
  const { reloadOP, modifyOP, delOP } = props;
  const [keyMatch, setKeyMatch] = useState('');
  const [resp, setResp] = useState<types.MuiDataGridData | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [selectRow, setSelectRow] = useState(null);
  const [loading, setLoading] = useState(false);
  const handleReload = (showOK) => {
    setLoading(true);
    reloadOP(keyMatch)
      .then((resp) => {
        resp.data.forEach((e) => {
          e.id = uuidv4();
        });
        resp.fields[0]['flex'] = 1;
        resp.fields[0]['renderCell'] = (p) => {
          return redisRenderCell(p.row.value);
        };
        setResp(resp);
        if (showOK) {
          reloadOK(showNotification);
        }
      })
      .catch((e) => {
        showNotification('error', 'load data error', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  useEffect(() => {
    handleReload(false);
  }, []);

  const handleDel = async (id: string) => {
    let del = resp.data.find((e) => e['id'] == id);
    try {
      await delOP(del.value);
      showNotification('success', 'Delete OK');
    } catch (e) {
      showNotification('error', 'delete failure', e);
    }

    handleReload(false);
  };
  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);

  const vis = useContext(VisiableContext);
  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          let content = (target as HTMLDivElement).innerHTML;
          let id = (currentTarget as HTMLDivElement).getAttribute('data-id');
          let field = (target as HTMLDivElement).getAttribute('data-field');
          if (!field) {
            return;
          }
          setUpMenu({
            content: (
              <RedisSetTableContextMenu
                content={content}
                delOP={() => {
                  handleDel(id);
                }}
              ></RedisSetTableContextMenu>
            ),
            style: calMenuStyle(e, 200),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);
  const settings = useContext(SettingContext);
  const gridStyle = useMemo(() => {
    return { height: 'auto', maxHeight: '78vh' } as CSSProperties;
  }, []);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} alignItems="center">
        <TextField
          size="small"
          label="key match could be *key,key*,*key*"
          variant="outlined"
          value={keyMatch}
          onChange={(event) => {
            setKeyMatch(event.target.value);
          }}
          style={{ width: '35%' }}
        />
        <Button
          variant="contained"
          color="primary"
          onClick={() => {
            handleReload(true);
          }}
          loading={loading}
        >
          reload
        </Button>
        <DoubleClickToModify></DoubleClickToModify>
      </Stack>

      {resp && vis && (
        <div
          style={{
            display: 'grid',
          }}
        >
          <DataGrid
            style={gridStyle}
            showCellVerticalBorder
            showColumnVerticalBorder
            slotProps={slotProps}
            rowHeight={settings.datagridRowHeight}
            columnHeaderHeight={settings.datagridHeaderHeight}
            rows={resp.data}
            editMode="row"
            checkboxSelection={false}
            onRowDoubleClick={onRowDoubleClick}
            columns={resp.fields}
            disableColumnFilter
          ></DataGrid>
        </div>
      )}

      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setSelectRow(null);
          }}
        >
          <Box sx={modalStyle} width="80%">
            <RedisEditSet
              modifyOP={async (oldValue, newValue) => {
                try {
                  await modifyOP(oldValue, newValue);
                } catch (e) {
                  throw e;
                }
                //@ts-ignore
                selectRow.value = newValue;
                handleReload(false);
              }}
              //@ts-ignore
              key={selectRow.id}
              //@ts-ignore
              oldValue={selectRow.value}
              handleClose={() => {
                setSelectRow(null);
              }}
            ></RedisEditSet>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function RedisListTableContextMenu(props: {
  delOP: (count: number) => Promise<void>;
  content;
}) {
  const { delOP, content } = props;
  const [count, setCount] = useState('1');
  const [showDel, setShowDel] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  return (
    <Paper>
      <MenuList>
        {content && (
          <MenuItem
            onClick={() => {
              handleCopyToClipboard(content, showNotification);
            }}
          >
            <ListItemIcon>
              <ContentCopyIcon></ContentCopyIcon>
            </ListItemIcon>
            <ListItemText> Copy </ListItemText>
          </MenuItem>
        )}

        <MenuItem
          onClick={() => {
            setShowDel(true);
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText> Delete </ListItemText>
        </MenuItem>
      </MenuList>

      {showDel && (
        <Modal
          open={!!showDel}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setShowDel(null);
          }}
        >
          <Box sx={modalStyle} width="40%">
            <Stack spacing={1}>
              <TextField
                size="small"
                value={count}
                label="delete count"
                onChange={(e) => {
                  setCount(e.target.value);
                }}
                helperText=<span
                  style={{ cursor: 'pointer' }}
                  onClick={() => {
                    openUrl('https://redis.io/docs/latest/commands/lrem/');
                  }}
                >
                  count???
                </span>
                type="number"
              ></TextField>
              <Stack direction="row">
                <Button
                  onClick={async () => {
                    await delOP(parseInt(count));
                    setShowDel(false);
                  }}
                >
                  delete
                </Button>
              </Stack>
            </Stack>
          </Box>
        </Modal>
      )}
    </Paper>
  );
}

function RedisListTable(props: {
  reloadOP: () => Promise<types.MuiDataGridData>;
  modifyOP: (index: number, newValue: string) => Promise<void>;
  delOP: (count: number, value: string) => Promise<void>;
}) {
  const { reloadOP, modifyOP, delOP } = props;
  const [resp, setResp] = useState<types.MuiDataGridData | null>(null);
  const [resp2, setResp2] = useState<types.MuiDataGridData | null>(null);
  const [selectRow, setSelectRow] = useState<any>(null);
  const { showNotification } = useContext(NotificationContext);
  const [pageSize, setPageSize] = useState(useContext(SettingContext).pageSize);
  const [page, setPage] = useState(0);
  const [loading, setLoading] = useState(false);

  const handleReload = (showReloadOK: boolean) => {
    setLoading(true);
    reloadOP()
      .then((resp) => {
        resp.data.forEach((e) => {
          e.id = uuidv4();
        });
        resp.fields[1]['sortable'] = false;
        resp.fields[1]['flex'] = 1;
        resp.fields[0]['flex'] = 1;
        resp.fields[1]['renderCell'] = (p) => {
          return redisRenderCell(p.row.value);
        };
        setResp(resp);
        setResp2(resp);
        if (showReloadOK) {
          reloadOK(showNotification);
        }
      })
      .catch((e) => {
        showNotification('error', 'load data error', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  useEffect(() => {
    handleReload(false);
  }, []);

  const handleDel = async (id, deleteCount: number) => {
    let del = resp.data.find((e) => e.id == id);
    try {
      await delOP(deleteCount, del.value);
      showNotification('success', 'Delete OK');
    } catch (e) {
      showNotification('error', 'Delete failure', e);
    }
    handleReload(false);
  };

  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);

  const onPaginationModelChange = useCallback((x: GridPaginationModel) => {
    setPage(x.page);
    setPageSize(x.pageSize);
  }, []);
  const paginationModel = useMemo(() => {
    return { page, pageSize };
  }, [page, pageSize]);
  const onSortModelChange = useCallback(
    (c: GridSortModel) => {
      if (c.length == 0) {
        setResp(resp2);
        return;
      }
      let s = c[0];
      if (!s.sort) {
        setResp(resp2);
        return;
      }
      if (s.sort == 'asc') {
        setResp(resp2);
        return;
      }
      setResp({
        sql: resp2.sql,
        total: resp2.total,
        fields: resp2.fields,
        data: resp2.data.slice().reverse(),
      } as types.MuiDataGridData);
    },
    [resp2]
  );
  const vis = useContext(VisiableContext);
  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          let content = (target as HTMLDivElement).innerHTML;
          let id = (currentTarget as HTMLDivElement).getAttribute('data-id');
          let field = (target as HTMLDivElement).getAttribute('data-field');
          if (!field) {
            return;
          }
          setUpMenu({
            content: (
              <RedisListTableContextMenu
                delOP={async (count) => {
                  handleDel(id, count);
                }}
                content={content}
              ></RedisListTableContextMenu>
            ),
            style: calMenuStyle(e, 200),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);
  const setting = useContext(SettingContext);
  const gridStyle = useMemo(() => {
    return { height: 'auto', maxHeight: '78vh' } as CSSProperties;
  }, []);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1}>
        <Button
          variant="contained" // 可以是 'text', 'outlined', 或 'contained'
          color="primary" // 主题颜色
          onClick={() => {
            handleReload(true);
          }} // 点击事件处理
          loading={loading}
        >
          reload
        </Button>
        <DoubleClickToModify></DoubleClickToModify>
      </Stack>
      {resp && vis && (
        <div
          style={{
            display: 'grid',
          }}
        >
          <DataGrid
            loading={loading}
            style={gridStyle}
            showCellVerticalBorder
            showColumnVerticalBorder
            slotProps={slotProps}
            rowHeight={setting.datagridRowHeight}
            columnHeaderHeight={setting.datagridHeaderHeight}
            checkboxSelection={false}
            disableMultipleRowSelection
            disableColumnFilter
            rows={resp.data}
            columns={resp.fields}
            paginationModel={paginationModel}
            onRowDoubleClick={onRowDoubleClick}
            pageSizeOptions={pageSizeOptions}
            onPaginationModelChange={onPaginationModelChange}
            sortingMode="server"
            paginationMode="client"
            onSortModelChange={onSortModelChange}
          ></DataGrid>
        </div>
      )}

      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setSelectRow(null);
          }}
        >
          <Box sx={modalStyle} width="80%">
            <RedisEditList
              modifyOP={async (index, newValue) => {
                try {
                  await modifyOP(index, newValue);
                } catch (e) {
                  throw e;
                }
                handleReload(false);
                selectRow.value = newValue;
              }}
              key={selectRow.id}
              oldValue={selectRow.value}
              index={parseInt(selectRow.seq)}
              handleClose={() => {
                setSelectRow(null);
              }}
            ></RedisEditList>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function RedisZSetTableContextMenu(props: { delOP: () => void; content }) {
  const { delOP, content } = props;
  const { showNotification } = useContext(NotificationContext);
  return (
    <Paper>
      <MenuList>
        {content && (
          <MenuItem
            onClick={() => {
              handleCopyToClipboard(content, showNotification);
            }}
          >
            <ListItemIcon>
              <ContentCopyIcon></ContentCopyIcon>
            </ListItemIcon>
            <ListItemText> Copy </ListItemText>
          </MenuItem>
        )}

        <MenuItem
          onClick={() => {
            delOP();
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText> Delete </ListItemText>
        </MenuItem>
      </MenuList>
    </Paper>
  );
}

function RedisZsetTable(props: {
  reloadOP: (
    keyMatch: string,
    min: number | null,
    max: number | null
  ) => Promise<types.MuiDataGridData>;

  modifyOP: (
    oldMember: string,
    newKey: string,
    newValue: number
  ) => Promise<void>;

  delOP: (key: string) => Promise<void>;
}) {
  const { reloadOP, modifyOP, delOP } = props;
  const [keyMatch, setKeyMatch] = useState('');
  const [resp, setResp] = useState<types.MuiDataGridData | null>(null);
  const [selectRow, setSelectRow] = useState<any>();

  const [min, setMin] = useState('');
  const [max, setMax] = useState('');
  const [minErr, setMinErr] = useState(false);
  const [maxErr, setMaxErr] = useState(false);
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const handleReload = (showOK: boolean) => {
    if (minErr || maxErr) {
      return;
    }
    let minValue = parseFloat(min);
    let maxValue = parseFloat(max);
    setLoading(true);
    reloadOP(keyMatch, minValue, maxValue)
      .then((resp) => {
        resp.data.forEach((e) => {
          e.id = uuidv4();
        });
        resp.fields[0]['flex'] = 1;
        resp.fields[1]['flex'] = 1;
        resp.fields[0]['renderCell'] = (p) => {
          return redisRenderCell(p.row.key);
        };
        resp.data.forEach((e) => {
          //@ts-ignore rewrite to float value
          e.score = parseFloat(e['score']);
        });
        setResp(resp);
        if (showOK) {
          reloadOK(showNotification);
        }
      })
      .catch((e) => {
        showNotification('error', 'load data error', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  useEffect(() => {
    handleReload(false);
  }, []);

  const handleDel = async (id: string) => {
    let del = resp.data.find((e) => e['id'] == id);
    try {
      await delOP(del.key);
      showNotification('success', 'Delete OK');
    } catch (e) {
      showNotification('error', 'delete failure', e);
    }

    handleReload(false);
  };
  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);

  const vis = useContext(VisiableContext);
  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          let content = (target as HTMLDivElement).innerHTML;
          let id = (currentTarget as HTMLDivElement).getAttribute('data-id');
          let field = (target as HTMLDivElement).getAttribute('data-field');
          if (!field) {
            return;
          }
          setUpMenu({
            content: (
              <RedisZSetTableContextMenu
                content={content}
                delOP={() => {
                  handleDel(id);
                }}
              ></RedisZSetTableContextMenu>
            ),
            style: calMenuStyle(e, 200),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);
  const setting = useContext(SettingContext);
  const gridStyle = useMemo(() => {
    return { height: 'auto', maxHeight: '78vh' } as CSSProperties;
  }, []);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <TextField
          size="small"
          label="key match could be *key,key*,*key*"
          variant="outlined"
          value={keyMatch}
          onChange={(event) => {
            setKeyMatch(event.target.value);
          }}
          style={{ width: '35%' }}
        />
        <TextField
          size="small"
          label="minimum value(float64)"
          variant="outlined"
          value={min}
          error={minErr}
          onChange={(event) => {
            if (event.target.value) {
              setMinErr(isNaN(parseFloat(event.target.value)));
            } else {
              setMinErr(false);
            }
            setMin(event.target.value);
          }}
        />
        <TextField
          size="small"
          label="maxiumn value(float64)"
          variant="outlined"
          value={max}
          error={maxErr}
          onChange={(event) => {
            if (event.target.value) {
              setMaxErr(isNaN(parseFloat(event.target.value)));
            } else {
              setMaxErr(false);
            }
            setMax(event.target.value);
          }}
        />
        <Button
          variant="contained"
          color="primary"
          onClick={() => {
            handleReload(true);
          }}
          loading={loading}
        >
          reload
        </Button>
        <DoubleClickToModify></DoubleClickToModify>
      </Stack>

      {resp && vis && (
        <div
          style={{
            display: 'grid',
          }}
        >
          <DataGrid
            style={gridStyle}
            loading={loading}
            showCellVerticalBorder
            showColumnVerticalBorder
            slotProps={slotProps}
            rowHeight={setting.datagridRowHeight}
            columnHeaderHeight={setting.datagridHeaderHeight}
            rows={resp.data}
            checkboxSelection={false}
            editMode="row"
            onRowDoubleClick={onRowDoubleClick}
            columns={resp.fields}
            disableColumnFilter
          ></DataGrid>
        </div>
      )}

      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setSelectRow(null);
          }}
        >
          <Box sx={modalStyle} width="80%">
            <RedisEditZset
              modifyOP={async (oldKey, newKey, value) => {
                try {
                  await modifyOP(oldKey, newKey, value);
                } catch (e) {
                  throw e;
                }
                handleReload(false);
              }}
              key={selectRow}
              oldValue={selectRow.score}
              oldKey={selectRow.key}
              handleClose={() => {
                setSelectRow(null);
              }}
            ></RedisEditZset>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function RedisHashTableContextMenu(props: { delOP: () => void; content }) {
  const { delOP, content } = props;

  const { showNotification } = useContext(NotificationContext);
  return (
    <Paper>
      <MenuList>
        {content && (
          <MenuItem
            onClick={() => {
              handleCopyToClipboard(content, showNotification);
            }}
          >
            <ListItemIcon>
              <ContentCopyIcon></ContentCopyIcon>
            </ListItemIcon>
            <ListItemText> Copy </ListItemText>
          </MenuItem>
        )}

        <MenuItem
          onClick={() => {
            delOP();
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText> Delete </ListItemText>
        </MenuItem>
      </MenuList>
    </Paper>
  );
}

function RedisHashTable(props: {
  reloadOP: (keyMatch: string) => Promise<types.MuiDataGridData>;
  modifyOP: (
    oldMember: string,
    newMember: string,
    newValue: string
  ) => Promise<void>;

  delOP: (key: string) => Promise<void>;
}) {
  const { reloadOP, modifyOP, delOP } = props;
  const [keyMatch, setKeyMatch] = useState('');
  const [resp, setResp] = useState<types.MuiDataGridData | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [selectRow, setSelectRow] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const handleReload = (showOK) => {
    setLoading(true);
    reloadOP(keyMatch)
      .then((resp) => {
        resp.data.forEach((e) => {
          e.id = uuidv4();
        });
        resp.fields[0]['flex'] = 1;
        resp.fields[1]['flex'] = 1;
        resp.fields[0]['renderCell'] = (p) => {
          return redisRenderCell(p.row.key);
        };
        resp.fields[1]['renderCell'] = (p) => {
          return redisRenderCell(p.row.value);
        };
        setResp(resp);
        if (showOK) {
          reloadOK(showNotification);
        }
      })
      .catch((e) => {
        showNotification('error', 'load data error', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  useEffect(() => {
    handleReload(false);
  }, []);

  const handleDel = async (id: string) => {
    let del = resp.data.find((e) => e['id'] == id);
    try {
      await delOP(del.key);
      showNotification('success', 'Delete OK');
    } catch (e) {
      showNotification('error', 'Delete failure', e);
    }

    handleReload(false);
  };
  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);

  const vis = useContext(VisiableContext);
  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          let content = (target as HTMLDivElement).innerHTML;
          let id = (currentTarget as HTMLDivElement).getAttribute('data-id');
          let field = (target as HTMLDivElement).getAttribute('data-field');
          if (!field) {
            return;
          }
          setUpMenu({
            content: (
              <RedisHashTableContextMenu
                content={content}
                delOP={() => {
                  handleDel(id);
                }}
              ></RedisHashTableContextMenu>
            ),
            style: calMenuStyle(e, 200),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);
  const setting = useContext(SettingContext);
  const gridStyle = useMemo(() => {
    return { height: 'auto', maxHeight: '78vh' } as CSSProperties;
  }, []);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} alignItems="center">
        <TextField
          size="small"
          label="key match could be *key,key*,*key*"
          variant="outlined" // 可以是 'outlined', 'filled', or 'standard'
          value={keyMatch}
          onChange={(event) => {
            setKeyMatch(event.target.value);
          }}
          style={{ width: '35%' }}
        />
        <Button
          variant="contained" // 可以是 'text', 'outlined', 或 'contained'
          color="primary" // 主题颜色
          onClick={() => {
            handleReload(true);
          }} // 点击事件处理
          loading={loading}
        >
          reload
        </Button>
        <DoubleClickToModify></DoubleClickToModify>
      </Stack>

      {resp && vis && (
        <div
          style={{
            display: 'grid',
          }}
        >
          <DataGrid
            loading={loading}
            style={gridStyle}
            showCellVerticalBorder
            showColumnVerticalBorder
            slotProps={slotProps}
            rowHeight={setting.datagridRowHeight}
            columnHeaderHeight={setting.datagridHeaderHeight}
            rows={resp.data}
            checkboxSelection={false}
            editMode="row"
            onRowDoubleClick={onRowDoubleClick}
            columns={resp.fields}
            disableColumnFilter
          ></DataGrid>
        </div>
      )}

      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setSelectRow(null);
          }}
        >
          <Box sx={modalStyle} width="80%">
            <RedisEditHash
              modifyOP={async (req) => {
                try {
                  await modifyOP(req.oldKey, req.newKey, req.value);
                } catch (e) {
                  throw e;
                }
                selectRow.key = req.newKey;
                selectRow.value = req.value;
                handleReload(false);
              }}
              handleClose={() => {
                setSelectRow(null);
              }}
              key={selectRow.id}
              oldValue={selectRow.value}
              oldKey={selectRow.key}
            ></RedisEditHash>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function RedisModifyKeyName(props: {
  name: string;
  modifyOP: (oldName: string, newName: string) => Promise<void>;
}) {
  let { name, modifyOP } = props;
  const [newName, setNewName] = useState(name);
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);

  const handleModify = () => {
    setLoading(true);
    modifyOP(newName, name)
      .then(() => {
        showNotification('success', 'Modify OK');
      })
      .catch((e) => {
        showNotification('error', 'Modify failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <Stack direction="row">
        <Typography variant="subtitle1"> New name: </Typography>
      </Stack>

      <TextField
        size="small"
        value={newName}
        onChange={(e) => {
          setNewName(e.target.value);
        }}
        label="name"
      ></TextField>
      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          disabled={name == newName}
          color="primary"
          variant="contained"
          onClick={handleModify}
        >
          Save
        </Button>
      </Stack>
    </Stack>
  );
}

function DoubleClickToModify() {
  return (
    <Typography color="info" variant="subtitle1">
      double click a row to modify.
    </Typography>
  );
}

export {
  RedisListTable,
  RedisStringTable,
  RedisHashTable,
  RedisZsetTable,
  RedisSetTable,
  RedisValueEditor,
  RedisModifyKeyName,
  // add
  RedisAddSet,
  RedisAddHash,
  RedisAddList,
  RedisAddZset,
};
