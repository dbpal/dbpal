import {
  RefObject,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
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
  Divider,
} from "@mui/material";

import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import { modalStyle, reloadOK, EscDownEvent, modalHandleClose } from "./Common";
import ClearIcon from "@mui/icons-material/Clear";
import { openUrl } from "@tauri-apps/plugin-opener";
import { NotificationContext } from "./Notification";
import {
  DataGrid,
  GridCallbackDetails,
  GridPaginationModel,
  GridSlotsComponent,
  GridSlotsComponentsProps,
  GridSortModel,
} from "@mui/x-data-grid";
import {
  ColumnWidth,
  ContextMenuContext,
  handleCopyToClipboard,
  hexToString,
  hexTryToString,
  ErrAndLoc,
  saveAndSetSize,
  SettingContext,
  stringToHex,
  validJson,
  SavePageSize,
  MuiDataGridData,
  extractFieldAndIDFromMouseEvent,
  dataGridStyle,
  pageSizeOptions,
  RedisTableType,
  DataSourceKind,
  newEmptyResp,
  secondConfirm,
} from "./helper.ts";
import {
  AppColumnMenu,
  ModalHeader,
  ResetColumnWidth,
  TimeDuration,
} from "./CommonComp.tsx";
import { editor, Position } from "monaco-editor";
import {
  SliderSize,
  SimpleFlexEditor,
  TableAddButton,
  AppDataGridPagination,
  AppDataGridToolBar,
  LastFetchTime,
  QueryStatus,
  StackRowCenter,
  getSliderSizeStoreValue,
} from "./CommonComp2.tsx";
import { invoke } from "@tauri-apps/api/core";
import { showTable } from "./handlers.ts";
import { AddTableView } from "./LeftTable.tsx";

class RedisZsetHashModifyReq {
  constructor(
    public oldKey: string,
    public newKey: string,
    public value: string,
  ) {}
}

enum RedisEditorMode {
  Hex = "hex",
  Text = "text",
  Json = "json",
}

type ListPushDirection = "left" | "right";
function RedisAddList(props: { uuid: string; listKey: string }) {
  const { uuid, listKey } = props;

  const lpushOP = async value => {
    return await invoke("redis_list_lpush", {
      name: uuid,
      key: listKey,
      value,
    });
  };
  const rpushOP = async value => {
    return await invoke("redis_list_rpush", {
      name: uuid,
      key: listKey,
      value,
    });
  };
  const [loading, setLoading] = useState(false);
  const [direction, setDirection] = useState<ListPushDirection>("left");
  const { showNotification } = useContext(NotificationContext);
  const valueRef = useRef(new EditorAndMode());
  const handleAdd = () => {
    const content = valueRef.current.getValue();
    if (!content) {
      showNotification("warning", "Please input value");
      return;
    }
    setLoading(true);
    (direction == "left" ? lpushOP : rpushOP)(content)
      .then(() => {
        showNotification("success", "Push OK");
      })
      .catch(e => {
        showNotification("error", "Push Failure", e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  return (
    <Stack style={{ height: "100%" }}>
      <StackRowCenter>
        <Typography color="info"> direction: </Typography>
        <RadioGroup
          row
          onChange={(_e, value) => {
            setDirection(value as ListPushDirection);
          }}
          value={direction}
        >
          <FormControlLabel value="left" control={<Radio />} label="head" />
          <FormControlLabel value="right" control={<Radio />} label="tail" />
        </RadioGroup>
      </StackRowCenter>

      <RedisValueEditor valueRef={valueRef} />

      <Stack direction="row">
        <Button loading={loading} color="primary" onClick={handleAdd}>
          push
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisStringTable(props: { uuid: string; stringKey: string }) {
  const { uuid, stringKey } = props;
  const modifyOP = async value => {
    return await invoke("redis_string_set", {
      name: uuid,
      key: stringKey,
      value,
    });
  };
  const [content, setContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const valueRef = useRef(new EditorAndMode());
  const [queryStatus, setQueryStatus] = useState("");
  useEffect(() => {
    showTable({
      uuid,
      name: stringKey,
      showNotification,
      handleResult: resp => {
        setContent(resp.data[0]["result"]);
      },
      setStatus: setQueryStatus,
      setLoading,
    });
  }, [showNotification, stringKey, uuid]);

  const handleModify = async () => {
    const confirm = await secondConfirm({ title: "sure to modify?" });
    if (!confirm) {
      return;
    }
    setLoading(true);
    modifyOP(valueRef.current.getValue())
      .then(() => {
        showNotification("success", "Modify OK");
      })
      .catch(e => {
        showNotification("error", "Modify failure", e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  return (
    <Stack
      style={{
        overflow: "auto",
        height: "100%",
      }}
    >
      <QueryStatus status={queryStatus} />
      {typeof content == "string" && (
        <RedisValueEditor value={content} valueRef={valueRef} />
      )}
      <Stack direction="row">
        <Button
          loading={loading}
          color="primary"
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

function RedisAddSet(props: { uuid: string; setKey: string }) {
  const { uuid, setKey } = props;

  const addOP = async member => {
    return await invoke("redis_set_add", {
      name: uuid,
      key: setKey,
      member,
    });
  };
  const [loading, setLoading] = useState(false);
  const valueRef = useRef(new EditorAndMode());
  const { showNotification } = useContext(NotificationContext);
  const handleAdd = () => {
    const c = valueRef.current.getValue();
    if (!c) {
      showNotification("warning", "please input value");
      return;
    }
    setLoading(true);
    addOP(c)
      .then(() => {
        showNotification("success", "Add ok");
      })
      .catch(e => {
        showNotification("error", "Add error", e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  return (
    <Stack style={{ height: "100%" }}>
      <RedisValueEditor valueRef={valueRef} />

      <Stack direction="row">
        <Button color="primary" loading={loading} onClick={handleAdd}>
          add
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisAddHash(props: { uuid: string; hashKey: string }) {
  const { uuid, hashKey } = props;

  const addOP = async (member, value) => {
    return await invoke("redis_hash_add", {
      name: uuid,
      key: hashKey,
      member,
      value,
    });
  };
  const [loading, setLoading] = useState(false);
  const key = useRef(new EditorAndMode());
  const value = useRef(new EditorAndMode());
  const { showNotification } = useContext(NotificationContext);
  const handleAdd = () => {
    setLoading(true);
    addOP(key.current.getValue(), value.current.getValue())
      .then(() => {
        showNotification("success", "Add OK");
      })
      .catch(e => {
        showNotification("error", "Add Error", e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack height={"100%"}>
      <div style={{ flex: 1, display: "flex", flexDirection: "column" }}>
        <RedisValueEditor value="" header="key" valueRef={key} />
      </div>
      <div style={{ flex: 1.5, display: "flex", flexDirection: "column" }}>
        <RedisValueEditor value="" valueRef={value} header="value" />
      </div>

      <Stack direction="row">
        <Button loading={loading} color="primary" onClick={handleAdd}>
          Add
        </Button>
      </Stack>
    </Stack>
  );
}
function RedisAddZset(props: { uuid: string; zsetKey: string }) {
  const { uuid, zsetKey } = props;

  const addOP = async (member, value) => {
    return await invoke("redis_zset_add", {
      name: uuid,
      key: zsetKey,
      member,
      value,
    });
  };

  const [loading, setLoading] = useState(false);
  const [score, setScore] = useState("");
  const valueRef = useRef(new EditorAndMode());
  const { showNotification } = useContext(NotificationContext);

  const handleAdd = () => {
    // reset some state
    setLoading(true);
    addOP(valueRef.current.getValue(), parseFloat(score))
      .then(() => {
        showNotification("success", "Add OK");
      })
      .catch(e => {
        showNotification("error", "Add Failure", e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack style={{ height: "100%" }}>
      <RedisValueEditor valueRef={valueRef} />
      <Stack direction="row" style={{ alignItems: "flex-start" }}>
        <Button loading={loading} color="primary" onClick={handleAdd}>
          Add
        </Button>
        <TextField
          value={score}
          label="score"
          error={score ? isNaN(parseFloat(score)) : false}
          helperText="put a valid float64 value"
          onChange={event => {
            setScore(event.target.value);
          }}
          slotProps={{
            input: {
              endAdornment: score && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setScore("");
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
  const newkey = useRef(new EditorAndMode());
  const newValue = useRef(new EditorAndMode());
  const { showNotification } = useContext(NotificationContext);

  const handleModify = async () => {
    const confirm = await secondConfirm({ title: "sure to modify?" });
    if (!confirm) {
      return;
    }
    setLoading(true);
    modifyOP(
      new RedisZsetHashModifyReq(
        oldKey,
        newkey.current.getValue(),
        newValue.current.getValue(),
      ),
    )
      .then(() => {
        showNotification("success", "Modify OK");
        handleClose();
      })
      .catch(e => {
        showNotification("error", "Modify failure", e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack flex={1}>
      <div style={{ flex: 1, display: "flex", flexDirection: "column" }}>
        <RedisValueEditor value={oldKey} valueRef={newkey} header="key" />
      </div>
      <div style={{ flex: 1.6, display: "flex", flexDirection: "column" }}>
        <RedisValueEditor value={oldValue} valueRef={newValue} header="value" />
      </div>

      <Stack direction="row">
        <Button loading={loading} color="primary" onClick={handleModify}>
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
  const newKey = useRef(new EditorAndMode());
  const [newValue, setNewValue] = useState(oldValue);
  const { showNotification } = useContext(NotificationContext);
  const handleModify = async () => {
    if (oldKey == newKey.current.getValue() && oldValue == newValue) {
      return;
    }
    const confirm = await secondConfirm({ title: "sure to modify?" });
    if (!confirm) {
      return;
    }
    setLoading(true);
    modifyOP(oldKey, newKey.current.getValue(), parseInt(newValue))
      .then(() => {
        showNotification("success", "Modify OK");
        handleClose();
      })
      .catch(e => {
        showNotification("error", "Modify Failure", e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack flex={1}>
      <RedisValueEditor value={oldKey} valueRef={newKey} />
      <Stack direction="row" style={{ alignItems: "flex-start" }}>
        <Button loading={loading} color="primary" onClick={handleModify}>
          modify
        </Button>
        <TextField
          value={newValue}
          label="score"
          error={newValue ? isNaN(parseFloat(newValue)) : false}
          helperText="put a valid float64 value"
          onChange={event => {
            setNewValue(event.target.value);
          }}
          slotProps={{
            input: {
              endAdornment: newValue && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setNewValue("");
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
      {[RedisEditorMode.Text, RedisEditorMode.Json, RedisEditorMode.Hex].map(e => {
        return (
          <Chip
            key={e}
            label={e}
            onClick={() => {
              handleChange(e);
            }}
            color={mode == e ? "success" : "default"}
          />
        );
      })}
    </>
  );
}

/// editor and edit mode use to get value
class EditorAndMode {
  editor: editor.IStandaloneCodeEditor;
  mode: RedisEditorMode;
  getValue() {
    switch (this.mode) {
      case RedisEditorMode.Hex:
        return this.editor.getValue();
      default:
        return stringToHex(this.editor.getValue());
    }
  }
}

function RedisValueEditor(props: {
  value?: string;
  valueRef: RefObject<EditorAndMode>;
  header?: string;
}) {
  const { value, valueRef, header } = props;
  const [newValue, setNewValue] = useState<string>("");
  const [editModel, setEditModel] = useState<RedisEditorMode>(RedisEditorMode.Text);
  const [jsonErr, setJsonErr] = useState<ErrAndLoc>(null);
  const [jsonErrPos, setJsonErrPos] = useState<Position>(null);
  useEffect(() => {
    const v = hexTryToString(value);
    if (typeof v === "string") {
      let isJson = false;
      try {
        isJson = typeof JSON.parse(v) == "object";
      } catch (e) {
        if (import.meta.env.DEV) {
          console.log("not valid json ", e, "we will use text mode");
        }
      }
      if (isJson) {
        setEditModel(RedisEditorMode.Json);
      } else {
        setEditModel(RedisEditorMode.Text);
      }
      setNewValue(v);
    } else {
      setEditModel(RedisEditorMode.Hex);
      setNewValue(value);
    }
  }, [value]);

  // check if json ok
  useEffect(() => {
    if (editModel != RedisEditorMode.Json) {
      setJsonErr(null);
      return;
    }
    validJson(newValue).then(e => {
      setJsonErr(e);
    });
  }, [newValue, editModel]);
  useEffect(() => {
    valueRef.current.mode = editModel;
  }, [editModel, valueRef]);
  return (
    <Stack flex={1}>
      <StackRowCenter>
        <Typography
          variant="h6"
          color="info"
          style={{ fontWeight: "bold", textDecoration: "underline" }}
        >
          {header}
        </Typography>
        <RedisEditModeSelect
          mode={editModel}
          handleChange={mode => {
            if (
              mode == RedisEditorMode.Hex &&
              (editModel == RedisEditorMode.Json || editModel == RedisEditorMode.Text)
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
        />
        {editModel == RedisEditorMode.Hex && (
          <>
            <span>looking for hex editor???</span>
            <Chip
              label="try hexed.it"
              onClick={() => {
                openUrl("https://hexed.it/");
              }}
            />
          </>
        )}
      </StackRowCenter>
      <SimpleFlexEditor
        language={editModel == RedisEditorMode.Json ? "json" : "text"}
        value={newValue}
        handleValue={v => {
          setNewValue(v);
        }}
        handleEditor={ed => {
          valueRef.current.editor = ed;
        }}
        errPos={jsonErrPos}
        barLeft={
          jsonErr ? (
            <Typography
              color="error"
              variant="body1"
              style={{ cursor: "pointer" }}
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
      />
      {editModel == RedisEditorMode.Hex && (
        <Stack>
          <Divider />
          <pre style={{ maxHeight: "10vh", overflow: "auto" }}>
            {hexToString(newValue)}
          </pre>
        </Stack>
      )}
    </Stack>
  );
}

function RedisEditList(props: {
  modifyOP: (newValue: string) => Promise<void>;
  oldValue: string;
  handleClose: () => void;
}) {
  const { oldValue, modifyOP, handleClose } = props;
  const [loading, setLoading] = useState(false);
  const newValue = useRef(new EditorAndMode());
  const { showNotification } = useContext(NotificationContext);

  // really modify the content.
  const handleModify = async () => {
    const confirm = await secondConfirm({ title: "sure to modify?" });
    if (!confirm) {
      return;
    }
    setLoading(true);
    modifyOP(newValue.current.getValue())
      .then(() => {
        showNotification("success", "Modify OK", "");
        handleClose();
      })
      .catch(e => {
        showNotification("error", "Modify Failure", e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  return (
    <Stack flex={1}>
      <RedisValueEditor value={oldValue} valueRef={newValue} />
      <Stack direction="row">
        <Button loading={loading} color="primary" onClick={handleModify}>
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
  const { oldValue, modifyOP, handleClose } = props;
  const [loading, setLoading] = useState(false);
  const newValue = useRef(new EditorAndMode());
  const { showNotification } = useContext(NotificationContext);

  const handleModify = async () => {
    const confirm = await secondConfirm({ title: "sure to modify?" });
    if (!confirm) {
      return;
    }
    setLoading(true);
    modifyOP(oldValue, newValue.current.getValue())
      .then(() => {
        showNotification("success", "Modify OK");
        handleClose();
      })
      .catch(e => {
        showNotification("error", "Modify Failure", e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack flex={1}>
      <RedisValueEditor value={oldValue} valueRef={newValue} />
      <Stack direction="row">
        <Button loading={loading} color="primary" onClick={handleModify}>
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
            <ContentCopyIcon />
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
          <DeleteForeverIcon />
        </ListItemIcon>
        <ListItemText> Delete </ListItemText>
      </MenuItem>
    </MenuList>
  );
}

function LimitMaxRowNotice() {
  return <Stack> limit the max row retrun. </Stack>;
}

const keyMatcherHelpText = "key matcher string,could be *key,key*,*key*";

function RedisSetTable(props: { setKey: string; uuid: string }) {
  const { setKey, uuid } = props;
  const modifyOP = async (old, newValue) => {
    return await invoke("redis_set_replace", {
      name: uuid,
      key: setKey,
      old,
      new: newValue,
    });
  };
  const delOP = useCallback(
    async (member: string) => {
      return await invoke("redis_set_del", {
        name: uuid,
        key: setKey,
        member,
      });
    },
    [setKey, uuid],
  );
  const [keyMatch, setKeyMatch] = useState("");
  const [resp, setResp] = useState<MuiDataGridData>(null);
  const [lastFetchTime, setLastFetchTime] = useState(new Date());
  useEffect(() => {
    setLastFetchTime(new Date());
  }, [resp]);
  const [fetchDur, setFetchDur] = useState(0);
  const { showNotification } = useContext(NotificationContext);
  const [selectRow, setSelectRow] = useState<{ id; value }>(null);
  const [loading, setLoading] = useState(false);
  const maxRowRef = useRef(getSliderSizeStoreValue(redisMaxRowKey, redisMinRow));
  const [queryStatus, setQueryStatus] = useState("");
  const handleReload = useCallback(
    showOK => {
      setLoading(true);
      showTable({
        uuid,
        name: setKey,
        setLoading,
        para: { limit: maxRowRef.current, where_str: keyMatch ? keyMatch : null },
        handleExecuteTime: t => {
          setFetchDur(t);
        },
        showNotification,
        okCallBack: () => {
          if (showOK) {
            reloadOK(showNotification);
          }
        },
        setStatus: setQueryStatus,
        handleResult: resp => {
          resp.data.forEach((e, index) => {
            e["id"] = index;
          });
          resp.fields[0]["flex"] = 1;
          resp.fields[0]["valueGetter"] = valueGetter;
          setResp(resp);
        },
      });
    },
    [keyMatch, setKey, showNotification, uuid],
  );

  useEffect(() => {
    handleReload(false);
  }, [handleReload]);

  const handleDel = useCallback(
    async (id: string) => {
      const confirm = await secondConfirm({ title: "sure to delete?" });
      if (!confirm) {
        return;
      }
      const del = resp.data.find(e => e["id"] == id);
      try {
        await delOP(del.value);
        showNotification("success", "Delete OK");
      } catch (e) {
        showNotification("error", "delete failure", e);
      }
      handleReload(false);
    },
    [delOP, handleReload, resp, showNotification],
  );
  const onRowDoubleClick = useCallback(e => {
    setSelectRow(e.row);
  }, []);

  const { setUpMenu } = useContext(ContextMenuContext);
  const setting = useContext(SettingContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: e => {
          const { target } = e;
          const { field, id } = extractFieldAndIDFromMouseEvent(e);
          if (field) {
            return;
          }
          const content = (target as HTMLDivElement).innerHTML;
          setUpMenu({
            content: (
              <RedisSetTableContextMenu
                content={content}
                delOP={() => {
                  handleDel(id);
                }}
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.export.csvDelimiter,
          includeHeaders: setting.export.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [handleDel, setUpMenu, setting.export.csvDelimiter, setting.export.includeHeader]);

  const [modifyModalBox, setModifyModalBox] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (modifyModalBox) {
      const clean = saveAndSetSize(modifyModalBox, "redis-edit");
      return clean;
    }
  }, [modifyModalBox]);
  const slots = useMemo(() => {
    return {
      columnMenu: AppColumnMenu,
      toolbar: () => {
        return <AppDataGridToolBar left=<></> title={setKey} />;
      },
      pagination: () => {
        return (
          <Stack
            direction="row"
            style={{
              alignItems: "center",
              justifyContent: "space-between",
              flex: 1,
              overflowX: "auto",
            }}
          >
            <Stack style={{ flex: 1, alignItems: "center" }} direction="row">
              <LastFetchTime date={lastFetchTime} />
              <TimeDuration dur={fetchDur} />
              <SliderSize
                storeKey={redisMaxRowKey}
                min={redisMinRow}
                max={50000}
                step={1000}
                handleValue={v => {
                  maxRowRef.current = v;
                }}
                notice=<LimitMaxRowNotice />
              />
              <DoubleClickToModify />
            </Stack>
            <AppDataGridPagination showPageSelect />
          </Stack>
        );
      },
    } as Partial<GridSlotsComponent>;
  }, [fetchDur, lastFetchTime, setKey]);
  return (
    <Stack style={{ height: "100%" }}>
      <QueryStatus status={queryStatus} />
      <StackRowCenter>
        <Button
          color="primary"
          onClick={() => {
            handleReload(true);
          }}
          loading={loading}
        >
          reload
        </Button>
        <Tooltip title={keyMatcherHelpText}>
          <TextField
            label="key matcher"
            value={keyMatch}
            onChange={event => {
              setKeyMatch(event.target.value);
            }}
            style={{ maxWidth: "15rem", flex: 1 }}
            slotProps={{
              input: {
                endAdornment: keyMatch && (
                  <InputAdornment position="end">
                    <IconButton
                      onClick={() => {
                        setKeyMatch("");
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
        </Tooltip>
        <AddTable uuid={uuid} redisKey={setKey} redisType={RedisTableType.Set} />{" "}
      </StackRowCenter>

      {resp && (
        <DataGrid
          slots={slots}
          slotProps={slotProps}
          rows={resp.data}
          editMode="row"
          checkboxSelection={false}
          onRowDoubleClick={onRowDoubleClick}
          columns={resp.fields}
          paginationMode="client"
          disableColumnFilter
          style={dataGridStyle}
          loading={loading}
        />
      )}

      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setSelectRow(null);
          })}
        >
          <Box
            sx={{ resize: "both", ...modalStyle }}
            width="80%"
            ref={setModifyModalBox}
            height="70vh"
          >
            <ModalHeader
              handleClose={() => {
                setSelectRow(null);
              }}
              title="Edit Set value"
            />
            <RedisEditSet
              modifyOP={async (oldValue, newValue) => {
                modifyOP(oldValue, newValue);
                const idx = resp.data.findIndex(e => e["id"] == selectRow.id);
                setResp({
                  ...resp,
                  data: resp.data.map((e, index) => {
                    if (index == idx) {
                      e.value = newValue;
                    }
                    return e;
                  }),
                });
              }}
              oldValue={selectRow.value}
              handleClose={() => {
                setSelectRow(null);
              }}
            />
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
  const [count, setCount] = useState("1");
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
              <ContentCopyIcon />
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
            <DeleteForeverIcon />
          </ListItemIcon>
          <ListItemText> Delete </ListItemText>
        </MenuItem>
      </MenuList>

      {showDel && (
        <Modal
          open={!!showDel}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowDel(null);
          })}
        >
          <Box sx={modalStyle} width="40%">
            <ModalHeader
              title="Delete Redis List"
              handleClose={() => {
                setShowDel(null);
              }}
            />
            <Stack>
              <TextField
                value={count}
                label="delete count"
                onChange={e => {
                  setCount(e.target.value);
                }}
                helperText=<span
                  style={{ cursor: "pointer" }}
                  onClick={() => {
                    openUrl("https://redis.io/docs/latest/commands/lrem/");
                  }}
                >
                  count???
                </span>
                type="number"
              />
              <Stack direction="row">
                <Button
                  onClick={async () => {
                    await delOP(parseInt(count));
                    setShowDel(false);
                  }}
                  variant="outlined"
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

function AddTable(props: {
  uuid: string;
  redisKey: string;
  redisType: RedisTableType;
}) {
  const { redisKey, redisType, uuid } = props;
  const [showAdd, setShowAdd] = useState(false);
  const [addModalBox, setAddModalBox] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (addModalBox) {
      const clean = saveAndSetSize(addModalBox, "redis-add-" + redisType);
      return clean;
    }
  }, [addModalBox, redisType]);
  return (
    <>
      <TableAddButton handleAdd={() => setShowAdd(true)} />
      {showAdd && (
        <Modal
          open={showAdd}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowAdd(false);
          })}
        >
          <Box
            sx={{ resize: "both", ...modalStyle }}
            width="80%"
            height="70vh"
            ref={setAddModalBox}
          >
            <ModalHeader
              noTextTransform
              title={`add ${redisKey} `}
              handleClose={() => {
                setShowAdd(false);
              }}
            />
            <AddTableView
              uuid={uuid}
              table={redisKey}
              redisTableType={redisType}
              kind={DataSourceKind.Redis}
            />
          </Box>
        </Modal>
      )}
    </>
  );
}

function RedisListTable(props: { listKey: string; uuid: string }) {
  const { listKey, uuid } = props;
  const modifyOP = async (index, value) => {
    return await invoke("redis_list_set", {
      name: uuid,
      key: listKey,
      value,
      index,
    });
  };
  const delOP = useCallback(
    async (count, value) => {
      return await invoke("redis_list_remove", {
        name: uuid,
        key: listKey,
        value,
        count,
      });
    },
    [listKey, uuid],
  );
  const [resp, setResp] = useState<MuiDataGridData>(newEmptyResp());
  const [lastFetchTime, setLastFetchTime] = useState(new Date());
  useEffect(() => {
    setLastFetchTime(new Date());
  }, [resp]);
  const [fetchDur, setFetchDur] = useState(0);
  const [respOrigin, setRespOrigin] = useState<MuiDataGridData>(newEmptyResp());
  const [selectRow, setSelectRow] = useState<any>(null);
  const { showNotification } = useContext(NotificationContext);
  const [pageSize, setPageSize] = useState(SavePageSize.instance.get());
  const [page, setPage] = useState(0);
  const [loading, setLoading] = useState(false);
  const setting = useContext(SettingContext);
  const [queryStatus, setQueryStatus] = useState("");
  const handleReload = useCallback(
    (showOk: boolean) => {
      showTable({
        uuid,
        name: listKey,
        setLoading,
        showNotification,
        okCallBack: showOk
          ? () => {
              showNotification("success", "reload OK");
            }
          : undefined,
        handleExecuteTime: t => {
          setFetchDur(t);
        },
        handleResult: resp => {
          resp.data.forEach((e, index) => {
            e["id"] = index;
          });
          const seqWidth = ColumnWidth.instance.get("seq");
          if (seqWidth) {
            resp.fields[0]["width"] = seqWidth;
          }
          resp.fields[1]["sortable"] = false;
          resp.fields[1]["flex"] = 1;
          resp.fields[1]["valueGetter"] = valueGetter;
          setResp(resp);
          setRespOrigin(resp);
        },
        setStatus: setQueryStatus,
      });
    },
    [listKey, showNotification, uuid],
  );

  useEffect(() => {
    handleReload(false);
  }, [handleReload]);

  const handleDel = useCallback(
    async (id, deleteCount: number) => {
      const confirm = await secondConfirm({ title: "sure to delete?" });
      if (!confirm) {
        return;
      }
      const del = resp.data.find(e => e.id == id);
      try {
        await delOP(deleteCount, del.value);
        showNotification("success", "Delete OK");
      } catch (e) {
        showNotification("error", "Delete failure", e);
      }
      handleReload(false);
    },
    [delOP, handleReload, resp, showNotification],
  );

  const onRowDoubleClick = useCallback(e => {
    setSelectRow(e.row);
  }, []);

  const onPaginationModelChange = useCallback(
    (e: GridPaginationModel, reason: GridCallbackDetails<"pagination">) => {
      setPage(e.page);
      setPageSize(e.pageSize);
      if (reason.reason == "setPaginationModel") {
        SavePageSize.instance.save(e.pageSize);
      }
    },
    [],
  );

  const paginationModel = useMemo(() => {
    return { page, pageSize };
  }, [page, pageSize]);
  const onSortModelChange = useCallback(
    (c: GridSortModel) => {
      if (c.length == 0) {
        setResp(respOrigin);
        return;
      }
      const s = c[0];
      if (!s.sort) {
        setResp(respOrigin);
        return;
      }
      if (s.sort == "asc") {
        setResp(respOrigin);
        return;
      }
      setResp({
        sql: respOrigin.sql,
        total: respOrigin.total,
        fields: respOrigin.fields,
        data: respOrigin.data.slice().reverse(),
      } as MuiDataGridData);
    },
    [respOrigin],
  );

  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: e => {
          const { target } = e;
          const content = (target as HTMLDivElement).innerHTML;
          const { id, field } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          setUpMenu({
            content: (
              <RedisListTableContextMenu
                delOP={async count => {
                  handleDel(id, count);
                }}
                content={content}
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.export.csvDelimiter,
          includeHeaders: setting.export.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [handleDel, setUpMenu, setting.export.csvDelimiter, setting.export.includeHeader]);

  const onColumnResize = useCallback(({ colDef, width }) => {
    if (colDef.field == "seq") {
      ColumnWidth.instance.save(colDef.field, width);
    }
  }, []);

  const [modifyModalBox, setModifyModalBox] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (modifyModalBox) {
      const clean = saveAndSetSize(modifyModalBox, "redis-edit");
      return clean;
    }
  }, [modifyModalBox]);
  const slots = useMemo(() => {
    return {
      columnMenu: AppColumnMenu,
      toolbar: () => {
        return (
          <AppDataGridToolBar
            left=<>
              <Button
                color="primary"
                onClick={() => {
                  handleReload(true);
                }}
                loading={loading}
              >
                reload
              </Button>
              <AddTable
                uuid={uuid}
                redisKey={listKey}
                redisType={RedisTableType.List}
              />
            </>
            title={listKey}
          />
        );
      },
      pagination: () => {
        return (
          <Stack
            direction="row"
            style={{
              alignItems: "center",
              justifyContent: "space-between",
              flex: 1,
              overflowX: "auto",
            }}
          >
            <Stack style={{ alignItems: "center", flex: 1 }} direction="row">
              <LastFetchTime date={lastFetchTime} />
              <TimeDuration dur={fetchDur} />
              <DoubleClickToModify />
              <ResetColumnWidth />
            </Stack>

            <AppDataGridPagination showPageSelect />
          </Stack>
        );
      },
    } as Partial<GridSlotsComponent>;
  }, [fetchDur, handleReload, lastFetchTime, listKey, loading, uuid]);
  return (
    <Stack style={{ height: "100%" }}>
      <QueryStatus status={queryStatus} />
      {resp && (
        <DataGrid
          slots={slots}
          loading={loading}
          slotProps={slotProps}
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
          style={dataGridStyle}
          onSortModelChange={onSortModelChange}
        />
      )}
      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setSelectRow(null);
          })}
        >
          <Box
            sx={{ resize: "both", ...modalStyle }}
            width="80%"
            height="70vh"
            ref={setModifyModalBox}
          >
            <ModalHeader
              title="Edit List value"
              handleClose={() => {
                setSelectRow(null);
              }}
            />
            <RedisEditList
              modifyOP={async newValue => {
                const index = parseInt(selectRow.seq);
                modifyOP(index, newValue);
                setResp({
                  ...resp,
                  data: resp.data.map((e, idx) => {
                    if (index == idx) {
                      e.value = newValue;
                    }
                    return e;
                  }),
                });
              }}
              key={selectRow.id}
              oldValue={selectRow.value}
              handleClose={() => {
                setSelectRow(null);
              }}
            />
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
            <ContentCopyIcon />
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
          <DeleteForeverIcon />
        </ListItemIcon>
        <ListItemText> Delete </ListItemText>
      </MenuItem>
    </MenuList>
  );
}

const redisMaxRowKey = "redis-value";
const redisMinRow = 1000;

function RedisZsetTable(props: { uuid: string; zsetKey: string }) {
  const { uuid, zsetKey } = props;
  const modifyOP = async (oldMember: string, newKey: string, newValue: number) => {
    return await invoke("redis_zset_replace", {
      name: uuid,
      key: zsetKey,
      oldMember: oldMember,
      newMember: newKey,
      newValue,
    });
  };
  const delOP = useCallback(
    async member => {
      return await invoke("redis_zset_del", {
        name: uuid,
        key: zsetKey,
        member,
      });
    },
    [uuid, zsetKey],
  );
  const [keyMatch, setKeyMatch] = useState("");
  const [resp, setResp] = useState<MuiDataGridData>(newEmptyResp());

  const [lastFetchTime, setLastFetchTime] = useState(new Date());
  useEffect(() => {
    setLastFetchTime(new Date());
  }, [resp]);
  const [fetchDur, setFetchDur] = useState(0);

  const [selectRow, setSelectRow] = useState<any>();
  const [min, setMin] = useState("");
  const [max, setMax] = useState("");
  const [minErr, setMinErr] = useState(false);
  const [maxErr, setMaxErr] = useState(false);
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const setting = useContext(SettingContext);
  const maxRowRef = useRef(getSliderSizeStoreValue(redisMaxRowKey, redisMinRow));
  const [queryStatus, setQueryStatus] = useState("");
  const handleReload = useCallback(
    (showOK: boolean) => {
      if (minErr || maxErr) {
        return;
      }
      const minValue = parseFloat(min);
      const maxValue = parseFloat(max);
      setLoading(true);
      showTable({
        setStatus: setQueryStatus,
        uuid,
        name: zsetKey,
        setLoading,
        para: {
          min: minValue,
          max: maxValue,
          where_str: keyMatch ? keyMatch : null,
          limit: maxRowRef.current,
        },
        handleExecuteTime: t => {
          setFetchDur(t);
        },
        showNotification,
        okCallBack: showOK
          ? () => {
              showNotification("success", "reload OK");
            }
          : undefined,
        handleResult: resp => {
          resp.data.forEach((e, index) => {
            e["id"] = index;
          });
          resp.fields[1]["flex"] = 1;
          const scoreWidth = ColumnWidth.instance.get("score");
          if (scoreWidth) {
            resp.fields[0]["width"] = scoreWidth;
          }
          resp.data.forEach(e => {
            e.score = parseFloat(e["score"]);
          });
          resp.fields[1]["valueGetter"] = valueGetter;
          setResp(resp);
        },
      });
    },
    [keyMatch, max, maxErr, min, minErr, showNotification, uuid, zsetKey],
  );

  useEffect(() => {
    handleReload(false);
  }, [handleReload]);

  const handleDel = useCallback(
    async (id: string) => {
      const confirm = await secondConfirm({ title: "sure to delete?" });
      if (!confirm) {
        return;
      }
      const del = resp.data.find(e => e["id"] == id);
      try {
        await delOP(del.key);
        showNotification("success", "Delete OK");
      } catch (e) {
        showNotification("error", "delete failure", e);
      }
      handleReload(false);
    },
    [delOP, handleReload, resp, showNotification],
  );
  const onRowDoubleClick = useCallback(e => {
    setSelectRow(e.row);
  }, []);

  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: e => {
          const { target } = e;
          const content = (target as HTMLDivElement).innerHTML;
          const { id, field } = extractFieldAndIDFromMouseEvent(e);
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
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.export.csvDelimiter,
          includeHeaders: setting.export.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [handleDel, setUpMenu, setting.export.csvDelimiter, setting.export.includeHeader]);

  const [modifyModalBox, setModifyModalBox] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (modifyModalBox) {
      const clean = saveAndSetSize(modifyModalBox, "redis-edit");
      return clean;
    }
  }, [modifyModalBox]);

  const onColumnResize = useCallback(({ colDef, width }) => {
    if (colDef.field == "score") {
      ColumnWidth.instance.save(colDef.field, width);
    }
  }, []);
  const slots = useMemo(() => {
    return {
      columnMenu: AppColumnMenu,
      toolbar: () => {
        return <AppDataGridToolBar title={zsetKey} />;
      },
      pagination: () => {
        return (
          <Stack
            direction="row"
            style={{
              alignItems: "center",
              justifyContent: "space-between",
              flex: 1,
              overflowX: "auto",
            }}
          >
            <Stack direction="row" style={{ flex: 1, alignItems: "center" }}>
              <LastFetchTime date={lastFetchTime} />
              <TimeDuration dur={fetchDur} />
              <SliderSize
                max={50000}
                step={1000}
                storeKey={redisMaxRowKey}
                min={redisMinRow}
                notice=<LimitMaxRowNotice />
                handleValue={v => {
                  maxRowRef.current = v;
                }}
              />
              <DoubleClickToModify />
              <ResetColumnWidth />
            </Stack>

            <AppDataGridPagination showPageSelect />
          </Stack>
        );
      },
    } as Partial<GridSlotsComponent>;
  }, [fetchDur, lastFetchTime, zsetKey]);
  return (
    <Stack style={{ height: "100%" }}>
      <QueryStatus status={queryStatus} />
      <StackRowCenter>
        <Button
          color="primary"
          onClick={() => {
            handleReload(true);
          }}
          loading={loading}
        >
          reload
        </Button>

        <Tooltip title={keyMatcherHelpText}>
          <TextField
            label="key matcher"
            value={keyMatch}
            onChange={event => {
              setKeyMatch(event.target.value);
            }}
            slotProps={{
              input: {
                endAdornment: keyMatch && (
                  <InputAdornment position="end">
                    <IconButton
                      onClick={() => {
                        setKeyMatch("");
                      }}
                      edge="end"
                    >
                      <ClearIcon fontSize="small" />
                    </IconButton>
                  </InputAdornment>
                ),
              },
            }}
            style={{ width: "25%" }}
          />
        </Tooltip>

        <TextField
          label="minimum value"
          value={min}
          error={minErr}
          onChange={event => {
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
                      setMin("");
                    }}
                    edge="end"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
          style={{ width: "15%" }}
        />
        <TextField
          label="maximum value"
          value={max}
          error={maxErr}
          onChange={event => {
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
                      setMax("");
                    }}
                    edge="end"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
          style={{ width: "15%" }}
        />
        <AddTable uuid={uuid} redisKey={zsetKey} redisType={RedisTableType.Zset} />
      </StackRowCenter>

      {resp && (
        <DataGrid
          slots={slots}
          onColumnResize={onColumnResize}
          loading={loading}
          slotProps={slotProps}
          rows={resp.data}
          checkboxSelection={false}
          editMode="row"
          onRowDoubleClick={onRowDoubleClick}
          columns={resp.fields}
          disableColumnFilter
          style={dataGridStyle}
          paginationMode="client"
        />
      )}
      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setSelectRow(null);
          })}
        >
          <Box
            sx={{ resize: "both", ...modalStyle }}
            width="80%"
            height="70vh"
            ref={setModifyModalBox}
          >
            <ModalHeader
              title="Edit Zset Value"
              handleClose={() => {
                setSelectRow(null);
              }}
            />
            <RedisEditZset
              modifyOP={async (oldKey, newKey, value) => {
                modifyOP(oldKey, newKey, value);
                const idx = resp.data.findIndex(e => oldKey === e.key);
                setResp({
                  ...resp,
                  data: resp.data.map((e, index) => {
                    if (index == idx) {
                      e.key = newKey;
                      e.score = value;
                    }
                    return e;
                  }),
                });
              }}
              key={selectRow}
              oldValue={selectRow.score}
              oldKey={selectRow.key}
              handleClose={() => {
                setSelectRow(null);
              }}
            />
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
              <ContentCopyIcon />
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
            <DeleteForeverIcon />
          </ListItemIcon>
          <ListItemText> Delete </ListItemText>
        </MenuItem>
      </MenuList>
    </div>
  );
}

const valueGetter = value => {
  const x = hexTryToString(value);
  if (typeof x === "string") {
    return x;
  } else {
    return "binary value of " + value;
  }
};
function RedisHashTable(props: { hashKey: string; uuid: string }) {
  const { uuid, hashKey } = props;
  const delOP = useCallback(
    async member => {
      return await invoke("redis_hash_del", {
        name: uuid,
        key: hashKey,
        member,
      });
    },
    [hashKey, uuid],
  );

  const modifyOP = async (oldMember, newMember, newValue) => {
    return await invoke("redis_hash_replace", {
      name: uuid,
      key: hashKey,
      oldMember,
      newMember,
      newValue,
    });
  };
  const [keyMatch, setKeyMatch] = useState("");
  const [resp, setResp] = useState<MuiDataGridData>(newEmptyResp());
  const { showNotification } = useContext(NotificationContext);
  const [selectRow, setSelectRow] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const setting = useContext(SettingContext);
  const maxRowRef = useRef(getSliderSizeStoreValue(redisMaxRowKey, redisMinRow));
  const [lastFetchTime, setLastFetchTime] = useState(new Date());
  useEffect(() => {
    setLastFetchTime(new Date());
  }, [resp]);
  const [fetchDur, setFetchDur] = useState(0);
  const [queryStatus, setQueryStatus] = useState("");

  const handleReload = useCallback(
    showOK => {
      showTable({
        uuid,
        name: hashKey,
        okCallBack: showOK
          ? () => {
              showNotification("success", "reload OK");
            }
          : undefined,
        handleExecuteTime: t => {
          setFetchDur(t);
        },
        setStatus: setQueryStatus,
        para: { limit: maxRowRef.current, where_str: keyMatch ? keyMatch : null },
        setLoading,
        showNotification,
        handleResult: resp => {
          resp.data.forEach((e, index) => {
            e["id"] = index;
          });
          const keyWidth = ColumnWidth.instance.get("key");
          resp.fields[0]["width"] = keyWidth;
          resp.fields[1]["flex"] = 1;
          resp.fields[0]["valueGetter"] = valueGetter;
          resp.fields[1]["valueGetter"] = valueGetter;
          setResp(resp);
        },
      });
    },
    [hashKey, keyMatch, showNotification, uuid],
  );
  useEffect(() => {
    handleReload(false);
  }, [handleReload]);

  const handleDel = useCallback(
    async (id: string) => {
      const confirm = await secondConfirm({ title: "sure to delete?" });
      if (!confirm) {
        return;
      }
      const del = resp.data.find(e => e["id"] == id);
      try {
        await delOP(del.key);
        showNotification("success", "Delete OK");
      } catch (e) {
        showNotification("error", "Delete failure", e);
      }

      handleReload(false);
    },
    [delOP, handleReload, resp, showNotification],
  );
  const onRowDoubleClick = useCallback(e => {
    setSelectRow(e.row);
  }, []);

  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: e => {
          const { target } = e;
          const content = (target as HTMLDivElement).innerHTML;
          const { id, field } = extractFieldAndIDFromMouseEvent(e);
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
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.export.csvDelimiter,
          includeHeaders: setting.export.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [handleDel, setUpMenu, setting.export.csvDelimiter, setting.export.includeHeader]);

  const [modifyModalBox, setModifyModalBox] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (modifyModalBox) {
      const clean = saveAndSetSize(modifyModalBox, "redis-edit");
      return clean;
    }
  }, [modifyModalBox]);
  const onColumnResize = useCallback(({ colDef, width }) => {
    if (colDef.field == "key") {
      ColumnWidth.instance.save(colDef.field, width);
    }
  }, []);
  const slots = useMemo(() => {
    return {
      columnMenu: AppColumnMenu,
      toolbar: () => {
        return <AppDataGridToolBar left=<></> title={hashKey} />;
      },
      pagination: () => {
        return (
          <Stack
            direction="row"
            style={{
              alignItems: "center",
              justifyContent: "space-between",
              flex: 1,
              overflowX: "auto",
            }}
          >
            <StackRowCenter>
              <LastFetchTime date={lastFetchTime} />
              <TimeDuration dur={fetchDur} />
              <SliderSize
                storeKey={redisMaxRowKey}
                min={redisMinRow}
                max={50000}
                step={1000}
                handleValue={v => {
                  maxRowRef.current = v;
                }}
                notice=<LimitMaxRowNotice />
              />

              <DoubleClickToModify />

              <ResetColumnWidth />
            </StackRowCenter>
            <Stack direction="row" alignItems="center">
              <AppDataGridPagination showPageSelect />
            </Stack>
          </Stack>
        );
      },
    } as Partial<GridSlotsComponent>;
  }, [fetchDur, hashKey, lastFetchTime]);
  return (
    <Stack style={{ height: "100%" }}>
      <QueryStatus status={queryStatus} />
      <StackRowCenter>
        <Button
          color="primary"
          onClick={() => {
            handleReload(true);
          }} // 点击事件处理
          loading={loading}
        >
          reload
        </Button>

        <Tooltip title={keyMatcherHelpText}>
          <TextField
            label="key matcher"
            variant="outlined"
            value={keyMatch}
            onChange={event => {
              setKeyMatch(event.target.value);
            }}
            slotProps={{
              input: {
                endAdornment: keyMatch && (
                  <InputAdornment position="end">
                    <IconButton
                      onClick={() => {
                        setKeyMatch("");
                      }}
                      edge="end"
                    >
                      <ClearIcon fontSize="small" />
                    </IconButton>
                  </InputAdornment>
                ),
              },
            }}
            style={{ maxWidth: "15rem", flex: 1 }}
          />
        </Tooltip>
        <AddTable uuid={uuid} redisKey={hashKey} redisType={RedisTableType.Hash} />
      </StackRowCenter>

      {resp && (
        <DataGrid
          slots={slots}
          onColumnResize={onColumnResize}
          loading={loading}
          slotProps={slotProps}
          rows={resp.data}
          checkboxSelection={false}
          editMode="row"
          onRowDoubleClick={onRowDoubleClick}
          columns={resp.fields}
          paginationMode="client"
          disableColumnFilter
          style={dataGridStyle}
        />
      )}

      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setSelectRow(null);
          })}
        >
          <Box
            sx={{ resize: "both", ...modalStyle }}
            width="80%"
            height="70vh"
            ref={setModifyModalBox}
          >
            <ModalHeader
              title="Edit Hash Value"
              handleClose={() => {
                setSelectRow(null);
              }}
            />
            <RedisEditHash
              modifyOP={async req => {
                modifyOP(req.oldKey, req.newKey, req.value);
                const idx = resp.data.findIndex(e => (e.key = req.oldKey));

                setResp({
                  ...resp,
                  data: resp.data.map((e, index) => {
                    if (index == idx) {
                      e.key = req.newKey;
                      e.value = req.newKey;
                    }
                    return e;
                  }),
                });
              }}
              handleClose={() => {
                setSelectRow(null);
              }}
              key={selectRow.id}
              oldValue={selectRow.value}
              oldKey={selectRow.key}
            />
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function DoubleClickToModify() {
  return (
    <Tooltip title="double click a row to modify.">
      <span> modify? </span>
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
  // add
  RedisAddSet,
  RedisAddHash,
  RedisAddList,
  RedisAddZset,
};
