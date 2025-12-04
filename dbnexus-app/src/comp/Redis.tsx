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
  GridSlotsComponentsProps,
  GridSortModel,
} from "@mui/x-data-grid";
import { BsFillLightbulbFill } from "react-icons/bs";
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
} from "./helper.ts";

import { ExecutingTime, ModalHeader } from "./CommonComp.tsx";
import { editor, Position } from "monaco-editor";
import { SliderSize, SimpleFlexEditor } from "./CommonComp2.tsx";
import { invoke } from "@tauri-apps/api/core";
import { showTable } from "./handlers.ts";

class RedisZsetHashModifyReq {
  constructor(public oldKey: string, public newKey: string, public value: string) {}
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
      <Stack direction="row" style={{ alignItems: "center" }}>
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
      </Stack>

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
  useEffect(() => {
    showTable({
      uuid,
      name: stringKey,
      isView: false,
      showNotification,
      handleResult: resp => {
        setContent(resp.data[0]["result"]);
      },
      setLoading,
    });
  }, [showNotification, stringKey, uuid]);

  const handleModify = () => {
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

  const handleModify = () => {
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
  const handleModify = () => {
    if (oldKey == newKey.current.getValue() && oldValue == newValue) {
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
      <Stack direction="row" style={{ alignItems: "center" }}>
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
      </Stack>
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
  modifyOP: (index: number, newValue: string) => Promise<void>;
  oldValue: string;
  index: number;
  handleClose: () => void;
}) {
  const { oldValue, modifyOP, handleClose, index } = props;
  const [loading, setLoading] = useState(false);
  const newValue = useRef(new EditorAndMode());
  const { showNotification } = useContext(NotificationContext);

  // really modify the content.
  const handleModify = () => {
    setLoading(true);
    modifyOP(index, newValue.current.getValue())
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

  const handleModify = () => {
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
  const [resp, setResp] = useState<MuiDataGridData | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [selectRow, setSelectRow] = useState<{ id; value }>(null);
  const [loading, setLoading] = useState(false);
  const maxRowRef = useRef<number>(2000);
  const handleReload = useCallback(
    showOK => {
      setLoading(true);
      showTable({
        uuid,
        name: setKey,
        isView: false,
        setLoading,
        para: { limit: maxRowRef.current },
        showNotification,
        handleResult: resp => {
          resp.data.forEach((e, index) => {
            e["id"] = index;
          });
          resp.fields[0]["flex"] = 1;
          resp.fields[0]["valueGetter"] = valueGetter;
          setResp(resp);
          if (showOK) {
            reloadOK(showNotification);
          }
        },
      });
    },
    [setKey, showNotification, uuid],
  );

  useEffect(() => {
    handleReload(false);
  }, [handleReload]);

  const handleDel = useCallback(
    async (id: string) => {
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

  const settings = useContext(SettingContext);
  const [modalBox, setModalBox] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (modalBox) {
      const clean = saveAndSetSize(modalBox, "redis-edit");
      return clean;
    }
  }, [modalBox]);

  return (
    <Stack style={{ height: "100%" }}>
      <Stack
        direction="row"
        style={{ alignItems: "center", justifyContent: "space-between" }}
      >
        <Stack direction="row" style={{ alignItems: "center", width: "50%" }}>
          <SliderSize
            storeKey="redis-set-table"
            valueRef={maxRowRef}
            min={1000}
            max={50000}
            step={1000}
            notice=<LimitMaxRowNotice />
          />
          <TextField
            label="key match could be *key,key*,*key*"
            value={keyMatch}
            onChange={event => {
              setKeyMatch(event.target.value);
            }}
            style={{ flex: 1 }}
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
          <Button
            color="primary"
            onClick={() => {
              handleReload(true);
            }}
            loading={loading}
          >
            reload
          </Button>
          <DoubleClickToModify />
        </Stack>

        <Stack direction="row" style={{ alignItems: "center" }}>
          <ExecutingTime loading={loading} title="loading time" />
        </Stack>
      </Stack>

      {resp && (
        <DataGrid
          showToolbar
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
            ref={setModalBox}
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
                handleReload(false);
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

  const [resp, setResp] = useState<MuiDataGridData | null>(null);
  const [respOrigin, setRespOrigin] = useState<MuiDataGridData | null>(null);
  const [selectRow, setSelectRow] = useState<any>(null);
  const { showNotification } = useContext(NotificationContext);
  const [pageSize, setPageSize] = useState(SavePageSize.instance.get());
  const [page, setPage] = useState(0);
  const [loading, setLoading] = useState(false);
  const setting = useContext(SettingContext);
  const handleReload = useCallback(
    (showReloadOK: boolean) => {
      showTable({
        uuid,
        name: listKey,
        isView: false,
        setLoading,
        showNotification,
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
          if (showReloadOK) {
            reloadOK(showNotification);
          }
        },
      });
    },
    [listKey, showNotification, uuid],
  );

  useEffect(() => {
    handleReload(false);
  }, [handleReload]);

  const handleDel = useCallback(
    async (id, deleteCount: number) => {
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

  const [modalBox, setModalBox] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (modalBox) {
      const clean = saveAndSetSize(modalBox, "redis-edit");
      return clean;
    }
  }, [modalBox]);
  const onColumnResize = useCallback(({ colDef, width }) => {
    if (colDef.field == "seq") {
      ColumnWidth.instance.save(colDef.field, width);
    }
  }, []);
  return (
    <Stack style={{ height: "100%" }}>
      <Stack
        direction="row"
        style={{ justifyContent: "space-between", alignItems: "center" }}
      >
        <Stack direction="row" style={{ alignItems: "center" }}>
          <Button
            color="primary"
            onClick={() => {
              handleReload(true);
            }}
            loading={loading}
          >
            reload
          </Button>
          <DoubleClickToModify />
        </Stack>

        <Stack direction="row" style={{ alignItems: "center" }}>
          <ExecutingTime loading={loading} title="loading time" />
        </Stack>
      </Stack>

      {resp && (
        <DataGrid
          showToolbar
          loading={loading}
          showCellVerticalBorder
          showColumnVerticalBorder
          slotProps={slotProps}
          rowHeight={setting.appearance.datagridRowHeight()}
          columnHeaderHeight={setting.appearance.datagridRowHeight()}
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
            ref={setModalBox}
          >
            <ModalHeader
              title="Edit List value"
              handleClose={() => {
                setSelectRow(null);
              }}
            />
            <RedisEditList
              modifyOP={async (index, newValue) => {
                modifyOP(index, newValue);

                handleReload(false);
              }}
              key={selectRow.id}
              oldValue={selectRow.value}
              index={parseInt(selectRow.seq)}
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
  const [resp, setResp] = useState<MuiDataGridData | null>(null);
  const [selectRow, setSelectRow] = useState<any>();

  const [min, setMin] = useState("");
  const [max, setMax] = useState("");
  const [minErr, setMinErr] = useState(false);
  const [maxErr, setMaxErr] = useState(false);
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const setting = useContext(SettingContext);
  const maxRowRef = useRef(2000);
  const handleReload = useCallback(
    (showOK: boolean) => {
      if (minErr || maxErr) {
        return;
      }
      const minValue = parseFloat(min);
      const maxValue = parseFloat(max);
      setLoading(true);
      showTable({
        uuid,
        name: zsetKey,
        isView: false,
        setLoading,
        para: {
          min: minValue,
          max: maxValue,
          where_str: keyMatch ? keyMatch : null,
          limit: maxRowRef.current,
        },
        showNotification,
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
          if (showOK) {
            reloadOK(showNotification);
          }
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

  const [modalBox, setModalBox] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (modalBox) {
      const clean = saveAndSetSize(modalBox, "redis-edit");
      return clean;
    }
  }, [modalBox]);
  const onColumnResize = useCallback(({ colDef, width }) => {
    if (colDef.field == "score") {
      ColumnWidth.instance.save(colDef.field, width);
    }
  }, []);
  return (
    <Stack style={{ height: "100%" }}>
      <Stack direction="row" style={{ alignItems: "center" }}>
        <SliderSize
          min={1000}
          max={50000}
          step={1000}
          storeKey="redis-zset-table"
          notice=<LimitMaxRowNotice />
          valueRef={maxRowRef}
        />
        <TextField
          label="key match could be *key,key*,*key*"
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
          style={{ flex: 1 }}
        />
        <TextField
          label="minimum value(float64)"
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
          style={{ flex: 0.5 }}
        />
        <TextField
          label="maximum value(float64)"
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
          style={{ flex: 0.5 }}
        />
        <Button
          color="primary"
          onClick={() => {
            handleReload(true);
          }}
          loading={loading}
        >
          reload
        </Button>
        <DoubleClickToModify />

        <ExecutingTime loading={loading} title="loading time" />
      </Stack>

      {resp && (
        <DataGrid
          showToolbar
          onColumnResize={onColumnResize}
          loading={loading}
          showCellVerticalBorder
          showColumnVerticalBorder
          slotProps={slotProps}
          rowHeight={setting.appearance.datagridRowHeight()}
          columnHeaderHeight={setting.appearance.datagridRowHeight()}
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
            ref={setModalBox}
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
                handleReload(false);
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
  const maxRowRef = useRef<number>(2000);
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
  const [resp, setResp] = useState<MuiDataGridData | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [selectRow, setSelectRow] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const setting = useContext(SettingContext);
  const handleReload = useCallback(
    showOK => {
      showTable({
        uuid,
        name: hashKey,
        isView: false,

        para: { limit: maxRowRef.current },
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
          if (showOK) {
            reloadOK(showNotification);
          }
        },
      });
    },
    [hashKey, showNotification, uuid],
  );

  useEffect(() => {
    handleReload(false);
  }, [handleReload]);

  const handleDel = useCallback(
    async (id: string) => {
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

  const [modalBox, setModalBox] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (modalBox) {
      const clean = saveAndSetSize(modalBox, "redis-edit");
      return clean;
    }
  }, [modalBox]);
  const onColumnResize = useCallback(({ colDef, width }) => {
    if (colDef.field == "key") {
      ColumnWidth.instance.save(colDef.field, width);
    }
  }, []);
  return (
    <Stack style={{ height: "100%" }}>
      <Stack
        direction="row"
        alignItems="center"
        style={{ justifyContent: "space-between" }}
      >
        <Stack direction="row" style={{ width: "55%", alignItems: "center" }}>
          <SliderSize
            storeKey="redis-hash-table"
            valueRef={maxRowRef}
            min={1000}
            max={50000}
            step={1000}
            notice=<LimitMaxRowNotice />
          />
          <TextField
            label="key match could be *key,key*,*key*"
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
            style={{ flex: 1 }}
          />
          <Button
            color="primary"
            onClick={() => {
              handleReload(true);
            }} // 点击事件处理
            loading={loading}
          >
            reload
          </Button>
          <DoubleClickToModify />
        </Stack>

        <Stack direction="row" style={{ alignItems: "center" }}>
          <ExecutingTime loading={loading} title="loading time" />
        </Stack>
      </Stack>

      {resp && (
        <DataGrid
          showToolbar
          onColumnResize={onColumnResize}
          loading={loading}
          showCellVerticalBorder
          showColumnVerticalBorder
          slotProps={slotProps}
          rowHeight={setting.appearance.datagridRowHeight()}
          columnHeaderHeight={setting.appearance.datagridRowHeight()}
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
            ref={setModalBox}
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
                handleReload(false);
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
  // add
  RedisAddSet,
  RedisAddHash,
  RedisAddList,
  RedisAddZset,
};
