import {
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import {
  Button,
  Stack,
  Box,
  TextField,
  MenuItem,
  Typography,
  Modal,
  MenuList,
  ListItemIcon,
  ListItemText,
  RadioGroup,
  Radio,
  FormControlLabel,
  Chip,
  Tooltip,
  InputAdornment,
  IconButton,
} from '@mui/material';
import { v4 as uuidv4 } from 'uuid';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import {
  modalStyle,
  reloadOK,
  SimpleFlexEditor,
  EscDownEvent,
} from './Common.tsx';
import ClearIcon from '@mui/icons-material/Clear';
import { types } from './ApiStruct.ts';
import { openUrl } from '@tauri-apps/plugin-opener';
import { NotificationContext } from './Notification';
import {
  DataGrid,
  GridCallbackDetails,
  GridPaginationModel,
  GridSlotsComponentsProps,
  GridSortModel,
} from '@mui/x-data-grid';
import { BsFillLightbulbFill } from 'react-icons/bs';
import {
  calMenuStyle,
  columnWidths,
  ContextMenuContext,
  gridStyle,
  handleCopyToClipboard,
  hexToString,
  hexTryToString,
  ErrAndLoc,
  saveAndSetSize,
  SettingContext,
  stringToHex,
  validJson,
  savePageSize,
} from './helper.ts';

import {
  ExecutingTime,
  ModalHeader,
  pageSizeOptions,
  ResizeWrapper,
} from './CommonComp.tsx';
import { Position } from 'monaco-editor';

class RedisZsetHashModifyReq {
  constructor(
    public oldKey: string,
    public newKey: string,
    public value: string
  ) {}
}

enum RedisEditorMode {
  Hex = 'hex',
  Text = 'text',
  Json = 'json',
}

function RedisAddList(props: {
  lpushOP: (value: string) => Promise<void>;
  rpushOP: (value: string) => Promise<void>;
}) {
  const { lpushOP, rpushOP } = props;
  const [loading, setLoading] = useState(false);
  const [direction, setDirection] = useState<'left' | 'right'>('left');
  const key = useRef('');
  const { showNotification } = useContext(NotificationContext);
  const handleAdd = () => {
    // reset some state
    setLoading(true);
    (direction == 'left' ? lpushOP : rpushOP)(key.current)
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
    <ResizeWrapper
      sizeKey="redis-add-list"
      sizeKeyKind="height"
      height="70vh"
      minHeight="50vh"
    >
      <Stack spacing={1} flex={1}>
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
          handleValue={(e) => {
            key.current = e;
          }}
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
    </ResizeWrapper>
  );
}

function RedisStringTable(props: {
  modifyOP: (content: string) => Promise<void>;
  loadContent: () => Promise<string>;
}) {
  const { modifyOP, loadContent } = props;
  const [content, setContent] = useState('');
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [loadOK, setLoadOK] = useState(false);
  const newContent = useRef('');
  useEffect(() => {
    loadContent()
      .then((e) => {
        newContent.current = e;
        setContent(e);
        setLoadOK(true);
      })
      .catch((e) => {
        showNotification('error', 'load redis string data failed', e);
      });
  }, []);

  const handleModify = () => {
    setLoading(true);
    modifyOP(newContent.current)
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
  const [dom, setDom] = useState(null);
  useEffect(() => {
    if (dom) {
      let c = saveAndSetSize(dom, 'redis-string-edit', 'height');
      return c;
    }
  }, [dom]);
  return (
    <Stack
      spacing={1}
      style={{
        resize: 'vertical',
        overflow: 'auto',
        height: '300px',
        minHeight: '20vh',
      }}
      ref={setDom}
    >
      {(content || loadOK) && (
        <RedisValueEditor
          oldValue={content}
          handleValue={(e) => {
            newContent.current = e;
          }}
        ></RedisValueEditor>
      )}
      <Stack direction="row" spacing={1}>
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
  const key = useRef('');

  const { showNotification } = useContext(NotificationContext);
  const handleAdd = () => {
    // reset some state
    setLoading(true);
    addOP(key.current)
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
    <ResizeWrapper
      sizeKey="redis-add-set"
      sizeKeyKind="height"
      height="70vh"
      minHeight="50vh"
    >
      <Stack spacing={1} flex={1}>
        <RedisValueEditor
          handleValue={(e) => {
            key.current = e;
          }}
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
    </ResizeWrapper>
  );
}

function RedisAddHash(props: {
  addOP: (key: string, value: string) => Promise<void>;
}) {
  let { addOP } = props;
  const [loading, setLoading] = useState(false);
  const value = useRef('');
  const key = useRef('');

  const { showNotification } = useContext(NotificationContext);
  const handleAdd = () => {
    // reset some state
    setLoading(true);
    addOP(key.current, value.current)
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
    <ResizeWrapper
      sizeKey="redis-add-hash"
      sizeKeyKind="height"
      height="70vh"
      minHeight="50vh"
    >
      <Stack spacing={1} flex={1}>
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
          <RedisValueEditor
            oldValue={key.current}
            handleValue={(e) => {
              key.current = e;
            }}
            header="key"
          ></RedisValueEditor>
        </div>
        <div style={{ flex: 1.5, display: 'flex', flexDirection: 'column' }}>
          <RedisValueEditor
            oldValue={value.current}
            handleValue={(e) => {
              value.current = e;
            }}
            header="value"
          ></RedisValueEditor>
        </div>

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
    </ResizeWrapper>
  );
}
function RedisAddZset(props: {
  addOP: (key: string, value: number) => Promise<void>;
}) {
  let { addOP } = props;
  const [loading, setLoading] = useState(false);
  const [score, setScore] = useState('');
  const key = useRef('');
  const [scoreErr, setScoreErr] = useState(false);

  const { showNotification } = useContext(NotificationContext);

  const handleAdd = () => {
    // reset some state
    setLoading(true);
    addOP(key.current, parseFloat(score))
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
    <ResizeWrapper
      sizeKey="redis-add-zset"
      sizeKeyKind="height"
      height="70vh"
      minHeight="50vh"
    >
      <Stack spacing={1} flex={1}>
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
          slotProps={{
            input: {
              endAdornment: score && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setScore('');
                    }}
                    edge="end"
                    size="small"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
        ></TextField>

        <RedisValueEditor
          handleValue={(e) => {
            key.current = e;
          }}
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
    </ResizeWrapper>
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
  const newkey = useRef(oldKey);
  const newValue = useRef(oldValue);
  const { showNotification } = useContext(NotificationContext);

  const handleModify = () => {
    // reset some state
    setLoading(true);
    modifyOP(
      new RedisZsetHashModifyReq(oldKey, newkey.current, newValue.current)
    )
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
    <Stack spacing={1} flex={1}>
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
        <RedisValueEditor
          oldValue={oldKey}
          handleValue={(e) => {
            newkey.current = e;
          }}
          header="key"
        ></RedisValueEditor>
      </div>
      <div style={{ flex: 1.5, display: 'flex', flexDirection: 'column' }}>
        <RedisValueEditor
          oldValue={oldValue}
          handleValue={(e) => {
            newValue.current = e;
          }}
          header="value"
        ></RedisValueEditor>
      </div>

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

  const newKey = useRef(oldKey);
  const [newValue, setNewValue] = useState(oldValue);
  const [newValueErr, setNewValueErr] = useState(false);
  const { showNotification } = useContext(NotificationContext);

  const handleModify = () => {
    if (oldKey == newKey.current && oldValue == newValue) {
      return;
    }
    setLoading(true);
    modifyOP(oldKey, newKey.current, parseInt(newValue))
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
    <Stack spacing={1} flex={1}>
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
        slotProps={{
          input: {
            endAdornment: newValue && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setNewValue('');
                  }}
                  edge="end"
                  size="small"
                >
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
        }}
      ></TextField>
      <RedisValueEditor
        oldValue={oldKey}
        handleValue={(e) => {
          newKey.current = e;
        }}
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
    <>
      {[RedisEditorMode.Text, RedisEditorMode.Json, RedisEditorMode.Hex].map(
        (e) => {
          return (
            <Chip
              key={e}
              label={e}
              onClick={() => {
                handleChange(e);
              }}
              size="small"
              color={mode == e ? 'primary' : 'default'}
            ></Chip>
          );
        }
      )}
    </>
  );
}

function RedisValueEditor(props: {
  oldValue?: string;
  handleValue: (a: string) => void;
  header?: string;
}) {
  let { oldValue, handleValue, header } = props;
  const [newValue, setNewValue] = useState<string>('');
  const [editModel, setEditModel] = useState<RedisEditorMode>(
    RedisEditorMode.Text
  );
  const [jsonErr, setJsonErr] = useState<ErrAndLoc>(null);
  const [jsonErrPos, setJsonErrPos] = useState<Position>(null);
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
    if (!newValue) {
      // newvalue is still null no need to set.
      return;
    }
    switch (editModel) {
      case RedisEditorMode.Hex:
        handleValue(newValue);
        break;
      default:
        handleValue(stringToHex(newValue));
    }
  }, [newValue]);

  useEffect(() => {
    if (editModel != RedisEditorMode.Json) {
      setJsonErr(null);
      return;
    }
    validJson(newValue)
      .then((e) => {
        setJsonErr(e);
      })
      .catch((e) => {
        console.log('call valid json error', e);
      });
  }, [newValue, editModel]);

  return (
    <Stack spacing={1} flex={1} style={{ background: '1px solod red' }}>
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <Typography
          variant="h6"
          color="info"
          style={{ fontWeight: 'bold', textDecoration: 'underline' }}
        >
          {header}
        </Typography>
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

        {editModel == RedisEditorMode.Hex && (
          <>
            <span>looking for hex editor???</span>
            <Chip
              label="try hexed.it"
              onClick={() => {
                openUrl('https://hexed.it/');
              }}
              size="small"
            ></Chip>
          </>
        )}
      </Stack>

      <SimpleFlexEditor
        language={editModel == RedisEditorMode.Json ? 'json' : 'text'}
        // in some times newvalue is not change as a string
        value={typeof newValue == 'object' ? '' : newValue}
        handleValue={(v) => {
          setNewValue(v);
        }}
        errPos={jsonErrPos}
        wordWrap="on"
        barLeft={
          jsonErr ? (
            <Typography
              color="error"
              variant="body1"
              style={{ cursor: 'pointer' }}
              onClick={() => {
                if (!jsonErr) {
                  return;
                }
                setJsonErrPos(new Position(jsonErr.line, jsonErr.column));
              }}
            >
              {jsonErr.msg}
            </Typography>
          ) : undefined
        }
      ></SimpleFlexEditor>
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
  const newValue = useRef(oldValue);
  const { showNotification } = useContext(NotificationContext);

  // really modify the content.
  const handleModify = () => {
    setLoading(true);
    modifyOP(index, newValue.current)
      .then(() => {
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
    <Stack spacing={1} flex={1}>
      <RedisValueEditor
        oldValue={oldValue}
        handleValue={(e) => {
          newValue.current = e;
        }}
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

function RedisEditSet(props: {
  modifyOP: (oldValue: string, newValue: string) => Promise<void>;
  oldValue: string;
  handleClose: () => void;
}) {
  let { oldValue, modifyOP, handleClose } = props;
  const [loading, setLoading] = useState(false);
  const newValue = useRef(oldValue);
  const { showNotification } = useContext(NotificationContext);

  // really modify the content.
  const handleModify = () => {
    setLoading(true);
    modifyOP(oldValue, newValue.current)
      .then(() => {
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
    <Stack spacing={1} flex={1} style={{ background: '1px solid red' }}>
      <RedisValueEditor
        oldValue={oldValue}
        handleValue={(e) => {
          newValue.current = e;
        }}
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

function RedisSetTableContextMenu(props: { delOP: () => void; content }) {
  const { delOP, content } = props;
  const { showNotification } = useContext(NotificationContext);
  return (
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
        resp.fields[0]['valueGetter'] = valueGetter;
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

  const { setUpMenu } = useContext(ContextMenuContext);
  const setting = useContext(SettingContext);
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
            style: calMenuStyle(e),
          });
        },
        style: { cursor: 'context-menu' },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.csvDelimiter,
          includeHeaders: setting.includeCsvHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);

  const settings = useContext(SettingContext);
  const [modalBox, setModalBox] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (modalBox) {
      let c = saveAndSetSize(modalBox, 'redis-edit');
      return c;
    }
  }, [modalBox]);

  return (
    <Stack spacing={1}>
      <Stack
        direction="row"
        spacing={1}
        style={{ alignItems: 'center', justifyContent: 'space-between' }}
      >
        <Stack
          spacing={1}
          direction="row"
          style={{ alignItems: 'center', width: '50%' }}
        >
          <TextField
            size="small"
            label="key match could be *key,key*,*key*"
            variant="outlined"
            value={keyMatch}
            onChange={(event) => {
              setKeyMatch(event.target.value);
            }}
            style={{ flex: 1 }}
            slotProps={{
              input: {
                endAdornment: keyMatch && (
                  <InputAdornment position="end">
                    <IconButton
                      onClick={() => {
                        setKeyMatch('');
                      }}
                      edge="end"
                      size="small"
                    >
                      <ClearIcon fontSize="small" />
                    </IconButton>
                  </InputAdornment>
                ),
              },
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

        <Stack direction="row" style={{ alignItems: 'center' }} spacing={1}>
          <ExecutingTime loading={loading} />
        </Stack>
        {resp && <NotReturnAllErr has_more={resp.has_more}></NotReturnAllErr>}
      </Stack>

      {resp && (
        <div
          style={{
            display: 'grid',
          }}
        >
          <DataGrid
            showToolbar
            style={gridStyle}
            showCellVerticalBorder
            showColumnVerticalBorder
            slotProps={slotProps}
            rowHeight={settings.datagridRowHeightXXX()}
            columnHeaderHeight={settings.datagridRowHeightXXX()}
            rows={resp.data}
            editMode="row"
            checkboxSelection={false}
            onRowDoubleClick={onRowDoubleClick}
            columns={resp.fields}
            paginationMode="client"
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
          <Box
            sx={{ resize: 'both', ...modalStyle }}
            width="80%"
            ref={setModalBox}
            height="70vh"
          >
            <ModalHeader title="Edit Set value"></ModalHeader>
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
    <div>
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
            <ModalHeader title="Delete Redis List"></ModalHeader>
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
    </div>
  );
}

function RedisListTable(props: {
  reloadOP: () => Promise<types.MuiDataGridData>;
  modifyOP: (index: number, newValue: string) => Promise<void>;
  delOP: (count: number, value: string) => Promise<void>;
}) {
  const { reloadOP, modifyOP, delOP } = props;
  const [resp, setResp] = useState<types.MuiDataGridData | null>(null);
  const [respOrigin, setRespOrigin] = useState<types.MuiDataGridData | null>(
    null
  );
  const [selectRow, setSelectRow] = useState<any>(null);
  const { showNotification } = useContext(NotificationContext);
  const [pageSize, setPageSize] = useState(savePageSize.get());
  const [page, setPage] = useState(0);
  const [loading, setLoading] = useState(false);

  const handleReload = (showReloadOK: boolean) => {
    setLoading(true);
    reloadOP()
      .then((resp) => {
        resp.data.forEach((e) => {
          e.id = uuidv4();
        });
        const seqWidth = columnWidths.getWidth('seq');
        if (seqWidth) {
          resp.fields[0]['width'] = seqWidth;
        }
        resp.fields[1]['sortable'] = false;
        resp.fields[1]['flex'] = 1;
        resp.fields[1]['valueGetter'] = valueGetter;
        setResp(resp);
        setRespOrigin(resp);
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

  const onPaginationModelChange = useCallback(
    (e: GridPaginationModel, reason: GridCallbackDetails<'pagination'>) => {
      if (reason.reason == 'setPaginationModel') {
        setPage(e.page);
        setPageSize(e.pageSize);
      }
    },
    []
  );
  useEffect(() => {
    savePageSize.save(pageSize);
  }, [pageSize]);
  const paginationModel = useMemo(() => {
    return { page, pageSize };
  }, [page, pageSize]);
  const onSortModelChange = useCallback(
    (c: GridSortModel) => {
      if (c.length == 0) {
        setResp(respOrigin);
        return;
      }
      let s = c[0];
      if (!s.sort) {
        setResp(respOrigin);
        return;
      }
      if (s.sort == 'asc') {
        setResp(respOrigin);
        return;
      }
      setResp({
        sql: respOrigin.sql,
        total: respOrigin.total,
        fields: respOrigin.fields,
        data: respOrigin.data.slice().reverse(),
      } as types.MuiDataGridData);
    },
    [respOrigin]
  );

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
            style: calMenuStyle(e),
          });
        },
        style: { cursor: 'context-menu' },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.csvDelimiter,
          includeHeaders: setting.includeCsvHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);
  const setting = useContext(SettingContext);

  const [modalBox, setModalBox] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (modalBox) {
      let c = saveAndSetSize(modalBox, 'redis-edit');
      return c;
    }
  }, [modalBox]);
  const onColumnResize = useCallback(({ colDef, width }) => {
    if (colDef.field == 'seq') {
      columnWidths.saveColumnWidths(colDef.field, width);
    }
  }, []);
  return (
    <Stack spacing={1}>
      <Stack
        direction="row"
        style={{ justifyContent: 'space-between', alignItems: 'center' }}
      >
        <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
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

        <Stack direction="row" style={{ alignItems: 'center' }} spacing={1}>
          <ExecutingTime loading={loading} />
        </Stack>
      </Stack>

      {resp && (
        <div
          style={{
            display: 'grid',
          }}
        >
          <DataGrid
            showToolbar
            loading={loading}
            style={gridStyle}
            showCellVerticalBorder
            showColumnVerticalBorder
            slotProps={slotProps}
            rowHeight={setting.datagridRowHeightXXX()}
            columnHeaderHeight={setting.datagridRowHeightXXX()}
            checkboxSelection={false}
            disableMultipleRowSelection
            disableColumnFilter
            rows={resp.data}
            onColumnResize={onColumnResize}
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
          <Box
            sx={{ resize: 'both', ...modalStyle }}
            width="80%"
            height="70vh"
            ref={setModalBox}
          >
            <ModalHeader title="Edit List value"></ModalHeader>
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
        resp.fields[1]['flex'] = 1;
        const scoreWidth = columnWidths.getWidth('score');

        if (scoreWidth) {
          resp.fields[0]['width'] = scoreWidth;
        }
        resp.data.forEach((e) => {
          //@ts-ignore rewrite to float value
          e.score = parseFloat(e['score']);
        });
        resp.fields[1]['valueGetter'] = valueGetter;
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
            style: calMenuStyle(e),
          });
        },
        style: { cursor: 'context-menu' },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.csvDelimiter,
          includeHeaders: setting.includeCsvHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);
  const setting = useContext(SettingContext);

  const [modalBox, setModalBox] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (modalBox) {
      let c = saveAndSetSize(modalBox, 'redis-edit');
      return c;
    }
  }, [modalBox]);
  const onColumnResize = useCallback(({ colDef, width }) => {
    if (colDef.field == 'score') {
      columnWidths.saveColumnWidths(colDef.field, width);
    }
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
          slotProps={{
            input: {
              endAdornment: keyMatch && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setKeyMatch('');
                    }}
                    edge="end"
                    size="small"
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
          slotProps={{
            input: {
              endAdornment: min && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setMin('');
                    }}
                    edge="end"
                    size="small"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
          style={{ flex: 0.5 }}
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
          slotProps={{
            input: {
              endAdornment: max && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setMax('');
                    }}
                    edge="end"
                    size="small"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
          style={{ flex: 0.5 }}
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
        {resp && <NotReturnAllErr has_more={resp.has_more}></NotReturnAllErr>}

        <ExecutingTime loading={loading} />
      </Stack>

      {resp && (
        <div
          style={{
            display: 'grid',
          }}
        >
          <DataGrid
            showToolbar
            style={gridStyle}
            onColumnResize={onColumnResize}
            loading={loading}
            showCellVerticalBorder
            showColumnVerticalBorder
            slotProps={slotProps}
            rowHeight={setting.datagridRowHeightXXX()}
            columnHeaderHeight={setting.datagridRowHeightXXX()}
            rows={resp.data}
            checkboxSelection={false}
            editMode="row"
            onRowDoubleClick={onRowDoubleClick}
            columns={resp.fields}
            disableColumnFilter
            paginationMode="client"
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
          <Box
            sx={{ resize: 'both', ...modalStyle }}
            width="80%"
            height="70vh"
            ref={setModalBox}
          >
            <ModalHeader title="Edit Zset Value"></ModalHeader>
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
    <div>
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
    </div>
  );
}

function NotReturnAllErr(props: { has_more: boolean }) {
  const { has_more } = props;
  return (
    has_more && (
      <Tooltip title="exceed max retrun value size,some result not retruned">
        <Typography color="error"> some key not found? </Typography>
      </Tooltip>
    )
  );
}

const valueGetter = (value) => {
  const x = hexTryToString(value);
  if (typeof x === 'string') {
    return x;
  } else {
    return 'binary value of' + value;
  }
};
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
        const keyWidth = columnWidths.getWidth('key');
        if (keyWidth) {
          resp.fields[0]['width'] = keyWidth;
        }
        resp.fields[1]['flex'] = 1;
        resp.fields[0]['valueGetter'] = valueGetter;
        resp.fields[1]['valueGetter'] = valueGetter;

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
            style: calMenuStyle(e),
          });
        },
        style: { cursor: 'context-menu' },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.csvDelimiter,
          includeHeaders: setting.includeCsvHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);
  const setting = useContext(SettingContext);

  const [modalBox, setModalBox] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (modalBox) {
      let c = saveAndSetSize(modalBox, 'redis-edit');
      return c;
    }
  }, [modalBox]);
  const onColumnResize = useCallback(({ colDef, width }) => {
    if (colDef.field == 'key') {
      columnWidths.saveColumnWidths(colDef.field, width);
    }
  }, []);
  return (
    <Stack spacing={1}>
      <Stack
        direction="row"
        spacing={1}
        alignItems="center"
        style={{ justifyContent: 'space-between' }}
      >
        <Stack
          direction="row"
          spacing={1}
          style={{ width: '55%', alignItems: 'center' }}
        >
          <TextField
            size="small"
            label="key match could be *key,key*,*key*"
            variant="outlined"
            value={keyMatch}
            onChange={(event) => {
              setKeyMatch(event.target.value);
            }}
            slotProps={{
              input: {
                endAdornment: keyMatch && (
                  <InputAdornment position="end">
                    <IconButton
                      onClick={() => {
                        setKeyMatch('');
                      }}
                      edge="end"
                      size="small"
                    >
                      <ClearIcon fontSize="small" />
                    </IconButton>
                  </InputAdornment>
                ),
              },
            }}
            style={{ flex: 1 }}
          />
          <Button
            variant="contained"
            color="primary"
            onClick={() => {
              handleReload(true);
            }} // 点击事件处理
            loading={loading}
          >
            reload
          </Button>
          <DoubleClickToModify></DoubleClickToModify>
          {resp && <NotReturnAllErr has_more={resp.has_more}></NotReturnAllErr>}
        </Stack>

        <Stack direction="row" style={{ alignItems: 'center' }} spacing={1}>
          <ExecutingTime loading={loading} />
        </Stack>
      </Stack>

      {resp && (
        <div
          style={{
            display: 'grid',
          }}
        >
          <DataGrid
            showToolbar
            onColumnResize={onColumnResize}
            loading={loading}
            style={gridStyle}
            showCellVerticalBorder
            showColumnVerticalBorder
            slotProps={slotProps}
            rowHeight={setting.datagridRowHeightXXX()}
            columnHeaderHeight={setting.datagridRowHeightXXX()}
            rows={resp.data}
            checkboxSelection={false}
            editMode="row"
            onRowDoubleClick={onRowDoubleClick}
            columns={resp.fields}
            paginationMode="client"
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
          <Box
            sx={{ resize: 'both', ...modalStyle }}
            width="80%"
            height="70vh"
            ref={setModalBox}
          >
            <ModalHeader title="Edit Hash Value"></ModalHeader>
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
        slotProps={{
          input: {
            endAdornment: newName && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setNewName('');
                  }}
                  edge="end"
                  size="small"
                >
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
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
    <Tooltip title="double click a row to modify.">
      <BsFillLightbulbFill />
    </Tooltip>
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
