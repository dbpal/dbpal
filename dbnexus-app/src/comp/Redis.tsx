import {
  Dispatch,
  RefObject,
  SetStateAction,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  Button,
  Stack,
  Box,
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
  Divider,
} from "@mui/material";

import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import { modalStyle, reloadOK, EscDownEvent, modalHandleClose } from "./Common";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ClearableTextField, FetchTimeDataGrid, RefreshButton } from "./CommonComp2";
import {
  handleCopyToClipboard,
  hexToString,
  hexTryToString,
  stringToHex,
  extractFieldAndIDFromMouseEvent,
  dataGridStyle,
  pageSizeOptions,
  RedisTableType,
  newEmptyResp,
  secondConfirm,
  DataSourceKind,
  AppColumnMenuCtx,
  AppColumnMenuKind,
  columnHeaderStyle,
  clickDataGridColumn,
  allGridHeaderTitle,
  gridColumnHeaderSelectClassName,
} from "./helper.ts";
import { AppColumnMenu, ModalHeader, ResetColumnWidth } from "./CommonComp.tsx";
import { editor } from "monaco-editor";
import {
  SliderSize,
  SimpleFlexEditor,
  AppDataGridPagination,
  AppDataGridToolBar,
  QueryStatus,
  StackRowCenter,
  ListenElementSize,
} from "./CommonComp2.tsx";
import { commands, MuiDataGridData } from "./bindings.ts";
import { showTable } from "./handlers.ts";
import { AddTableModal } from "./LeftTable.tsx";
import {
  useColumnWidthStore,
  useContextMenu,
  useNotification,
  usePageSize,
  UseSizeConfig,
  useSizeStore,
  useTableColumnVisibleStore,
} from "./store.tsx";
import {
  DataGrid,
  GridCallbackDetails,
  GridPaginationModel,
  GridSlotProps,
  GridSlotsComponent,
  GridSlotsComponentsProps,
  GridSortModel,
  useGridApiRef,
} from "./datagridExport.ts";
import { useDebounce } from "use-debounce";

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

type RedisMenuContent = string | null | undefined;

interface RedisContextMenuProps {
  delOP: () => void;
  content?: RedisMenuContent;
}

interface RedisListTableContextMenuProps {
  delOP: (count: number) => Promise<void>;
  content?: RedisMenuContent;
}

interface SetToolBarDbpalProps {
  keyMatch: string;
  handleReload: (showOk: boolean) => void;
  setKey: string;
  loading: boolean;
  setKeyMatch: Dispatch<SetStateAction<string>>;
  uuid: string;
}

interface ListToolBarDbpalProps {
  handleReload: (showOk: boolean) => void;
  listKey: string;
  loading: boolean;
  uuid: string;
}

interface ZSetToolBarDbpalProps {
  keyMatch: string;
  min: string;
  minErr: boolean;
  max: string;
  maxErr: boolean;
  loading: boolean;
  handleReload: (showOk: boolean) => void;
  setMinErr: Dispatch<SetStateAction<boolean>>;
  setKeyMatch: Dispatch<SetStateAction<string>>;
  setMin: Dispatch<SetStateAction<string>>;
  setMaxErr: Dispatch<SetStateAction<boolean>>;
  setMax: Dispatch<SetStateAction<string>>;
  uuid: string;
  zsetKey: string;
}

interface HashToolBarDbpalProps {
  keyMatch: string;
  setKeyMatch: Dispatch<SetStateAction<string>>;
  uuid: string;
  hashKey: string;
  loading: boolean;
  handleReload: (showOk: boolean) => void;
}

function RedisAddList(props: { uuid: string; listKey: string }) {
  const { uuid, listKey } = props;

  const lpushOP = async (value) => {
    return await commands.redisListLpush(uuid, listKey, value);
  };
  const rpushOP = async (value) => {
    return await commands.redisListRpush(uuid, listKey, value);
  };
  const [loading, setLoading] = useState(false);
  const [direction, setDirection] = useState<ListPushDirection>("left");
  const { showNotification } = useNotification();
  const valueRef = useRef(new EditorAndMode());
  const handleAdd = () => {
    const content = valueRef.current.getValue();
    if (!content) {
      showNotification({ severity: "warning", title: "Please input value" });
      return;
    }
    setLoading(true);
    (direction == "left" ? lpushOP : rpushOP)(content)
      .then(() => {
        showNotification({ severity: "success", title: "Push OK" });
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "Push Failure", message: e });
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
          value={direction}>
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
  const modifyOP = async (value) => {
    return await commands.redisStringSet(uuid, stringKey, value);
  };
  const [content, setContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const { showNotification } = useNotification();
  const valueRef = useRef(new EditorAndMode());
  const [queryStatus, setQueryStatus] = useState("");
  useEffect(() => {
    showTable({
      uuid,
      name: stringKey,
      showNotification,
      handleResult: (resp) => {
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
        showNotification({ severity: "success", title: "Modify OK" });
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "Modify failure", message: e });
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
      }}>
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
          }}>
          set
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisAddSet(props: { uuid: string; setKey: string }) {
  const { uuid, setKey } = props;

  const addOP = async (member) => {
    return await commands.redisSetAdd(uuid, setKey, member);
  };
  const [loading, setLoading] = useState(false);
  const valueRef = useRef(new EditorAndMode());
  const { showNotification } = useNotification();
  const handleAdd = () => {
    const c = valueRef.current.getValue();
    if (!c) {
      showNotification({ severity: "warning", title: "please input value" });
      return;
    }
    setLoading(true);
    addOP(c)
      .then(() => {
        showNotification({ severity: "success", title: "Add ok" });
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "Add error", message: e });
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
    return await commands.redisHashAdd(uuid, hashKey, member, value);
  };
  const [loading, setLoading] = useState(false);
  const key = useRef(new EditorAndMode());
  const value = useRef(new EditorAndMode());
  const { showNotification } = useNotification();
  const handleAdd = () => {
    setLoading(true);
    addOP(key.current.getValue(), value.current.getValue())
      .then(() => {
        showNotification({ severity: "success", title: "Add OK" });
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "Add Error", message: e });
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack sx={{ height: "100%" }}>
      <div style={{ flex: 1, display: "flex", flexDirection: "column" }}>
        <RedisValueEditor header="key" valueRef={key} />
      </div>
      <div style={{ flex: 1.5, display: "flex", flexDirection: "column" }}>
        <RedisValueEditor valueRef={value} header="value" />
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

  const addOP = async (member: string, value: number) => {
    return await commands.redisZsetAdd(uuid, zsetKey, member, value);
  };

  const [loading, setLoading] = useState(false);
  const [score, setScore] = useState("");
  const valueRef = useRef(new EditorAndMode());
  const { showNotification } = useNotification();

  const scoreRef = useRef<HTMLInputElement>(null);

  const handleAdd = () => {
    const scoreValue = parseFloat(score);
    if (isNaN(scoreValue)) {
      showNotification({
        severity: "warning",
        title: "score is not a number",
      });
      scoreRef.current?.focus();
      return;
    }
    setLoading(true);
    addOP(valueRef.current.getValue(), scoreValue)
      .then(() => {
        showNotification({ severity: "success", title: "Add OK" });
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "Add Failure", message: e });
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
        <ClearableTextField
          value={score}
          label="score"
          error={score ? isNaN(parseFloat(score)) : false}
          helperText="put a valid float64 value"
          onChange={(event) => {
            setScore(event.target.value);
          }}
          inputRef={scoreRef}
          onClear={() => setScore("")}
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
  const { showNotification } = useNotification();

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
        showNotification({ severity: "success", title: "Modify OK" });
        handleClose();
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "Modify failure", message: e });
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack style={{ flex: 1 }}>
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
  const { showNotification } = useNotification();
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
        showNotification({ severity: "success", title: "Modify OK" });
        handleClose();
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "Modify Failure", message: e });
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack style={{ flex: 1 }}>
      <RedisValueEditor value={oldKey} valueRef={newKey} />
      <Stack direction="row" style={{ alignItems: "flex-start" }}>
        <Button loading={loading} color="primary" onClick={handleModify}>
          modify
        </Button>
        <ClearableTextField
          value={newValue}
          label="score"
          error={newValue ? isNaN(parseFloat(newValue)) : false}
          helperText="put a valid float64 value"
          onChange={(event) => {
            setNewValue(event.target.value);
          }}
          onClear={() => setNewValue("")}
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
      {[RedisEditorMode.Text, RedisEditorMode.Json, RedisEditorMode.Hex].map((e) => {
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

  useEffect(() => {
    valueRef.current.mode = editModel;
  }, [editModel, valueRef]);
  return (
    <Stack style={{ flex: 1 }}>
      <StackRowCenter>
        <Typography
          variant="h6"
          color="info"
          style={{ fontWeight: "bold", textDecoration: "underline" }}>
          {header}
        </Typography>
        <RedisEditModeSelect
          mode={editModel}
          handleChange={(mode) => {
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
        handleValue={(v) => {
          setNewValue(v);
        }}
        handleEditor={(ed) => {
          valueRef.current.editor = ed;
        }}
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
  const { showNotification } = useNotification();

  // really modify the content.
  const handleModify = async () => {
    const confirm = await secondConfirm({ title: "sure to modify?" });
    if (!confirm) {
      return;
    }
    setLoading(true);
    modifyOP(newValue.current.getValue())
      .then(() => {
        showNotification({ severity: "success", title: "Modify OK", message: "" });
        handleClose();
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "Modify Failure", message: e });
      })
      .finally(() => {
        setLoading(false);
      });
  };
  return (
    <Stack style={{ flex: 1 }}>
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
  const { showNotification } = useNotification();

  const handleModify = async () => {
    const confirm = await secondConfirm({ title: "sure to modify?" });
    if (!confirm) {
      return;
    }
    setLoading(true);
    modifyOP(oldValue, newValue.current.getValue())
      .then(() => {
        showNotification({ severity: "success", title: "Modify OK" });
        handleClose();
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "Modify Failure", message: e });
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack style={{ flex: 1 }}>
      <RedisValueEditor value={oldValue} valueRef={newValue} />
      <Stack direction="row">
        <Button loading={loading} color="primary" onClick={handleModify}>
          modify
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisSetTableContextMenu(props: RedisContextMenuProps) {
  const { delOP, content } = props;
  const { showNotification } = useNotification();
  return (
    <MenuList>
      {content && (
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}>
          <ListItemIcon>
            <ContentCopyIcon />
          </ListItemIcon>
          <ListItemText> Copy </ListItemText>
        </MenuItem>
      )}

      <MenuItem
        onClick={() => {
          delOP();
        }}>
        <ListItemIcon>
          <DeleteForeverIcon />
        </ListItemIcon>
        <ListItemText> Delete </ListItemText>
      </MenuItem>
    </MenuList>
  );
}

const keyMatcherHelpText = "key matcher string,could be *key,key*,*key*";

type SetToolBarParameterProps = GridSlotProps["toolbar"] & {
  dbpal: SetToolBarDbpalProps;
};

function SetToolBar({
  dbpal: { keyMatch, setKey, handleReload, loading, setKeyMatch, uuid },
}: SetToolBarParameterProps) {
  return (
    <AppDataGridToolBar
      left=<>
        <RefreshButton
          color="primary"
          onClick={() => {
            handleReload(true);
          }}
          loading={loading}
        />
        <Tooltip title={keyMatcherHelpText}>
          <ClearableTextField
            label="key matcher"
            value={keyMatch}
            onChange={(event) => {
              setKeyMatch(event.target.value);
            }}
            onClear={() => setKeyMatch("")}
            style={{ maxWidth: "15rem", flex: 1 }}
          />
        </Tooltip>
        <AddTableModal
          kind={DataSourceKind.Redis}
          uuid={uuid}
          tableName={setKey}
          redisType={RedisTableType.Set}
        />
      </>
      title={setKey}
    />
  );
}

function RedisSetTable(props: { setKey: string; uuid: string }) {
  const { setKey, uuid } = props;
  const modifyOP = async (old, newValue) => {
    return await commands.redisSetReplace(uuid, setKey, old, newValue);
  };
  const apiRef = useGridApiRef();

  const [selectedColumns, setSelectColumns] = useState<Set<string>>(new Set());

  const delOP = useCallback(
    async (member: string) => {
      return await commands.redisSetDel(uuid, setKey, member);
    },
    [setKey, uuid],
  );
  const [keyMatch, setKeyMatch] = useState("");
  const [keyMatchDelay] = useDebounce(keyMatch, 1000);

  const [resp, setResp] = useState<MuiDataGridData>(null);
  const { showNotification } = useNotification();
  const [selectRow, setSelectRow] = useState<{ id; value }>(null);
  const [loading, setLoading] = useState(false);
  const { getSize } = useSizeStore();
  const [queryStatus, setQueryStatus] = useState("");
  const handleReload = useCallback(
    (showOK: boolean) => {
      setLoading(true);
      showTable({
        uuid,
        name: setKey,
        setLoading,
        para: {
          limit: getSize(UseSizeConfig.queryLimit),
          where_str: keyMatchDelay ? keyMatchDelay : null,
        },
        showNotification,
        okCallBack: () => {
          if (showOK) {
            reloadOK(showNotification);
          }
        },
        setStatus: setQueryStatus,
        handleResult: (resp) => {
          resp.data.forEach((e, index) => {
            e["id"] = index + "";
          });
          resp.fields[0]["flex"] = 1;
          resp.fields[0]["valueGetter"] = valueGetter;

          resp.fields.forEach((f) => {
            f["renderHeader"] = () => {
              return (
                <Stack
                  onClick={(event) => {
                    event.stopPropagation();
                    clickDataGridColumn(apiRef, event, f.field, setSelectColumns);
                  }}
                  style={{ ...columnHeaderStyle }}>
                  {f.field}
                </Stack>
              );
            };
          });
          setResp(resp);
        },
      });
    },
    [apiRef, getSize, keyMatchDelay, setKey, showNotification, uuid],
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
      const del = resp.data.find((e) => e["id"] == id);
      try {
        await delOP(del.value);
        showNotification({ severity: "success", title: "Delete OK" });
      } catch (e) {
        showNotification({ severity: "error", title: "delete failure", message: e });
      }
      handleReload(false);
    },
    [delOP, handleReload, resp, showNotification],
  );
  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);

  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target } = e;
          const { field, id } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
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
      pagination: { dbpal: { showLimit: true } },
      toolbar: {
        dbpal: (function (): SetToolBarParameterProps["dbpal"] {
          return { keyMatch, handleReload, setKey, loading, uuid, setKeyMatch };
        })(),
      },
    } as GridSlotsComponentsProps;
  }, [setUpMenu, handleDel, keyMatch, handleReload, setKey, loading, uuid]);

  const [modifyModalBox, setModifyModalBox] = useState<HTMLDivElement | null>(null);
  const slots = useMemo(() => {
    return {
      columnMenu: AppColumnMenu,
      toolbar: SetToolBar,
      pagination: RedisPagination,
    } as Partial<GridSlotsComponent>;
  }, []);

  const { getColumnVisibility, setColumnVisibility } = useTableColumnVisibleStore(
    (s) => s,
  );
  return (
    <Stack style={{ height: "100%" }}>
      <QueryStatus status={queryStatus} />

      {resp && (
        <AppColumnMenuCtx
          value={{
            selectColumns: (function () {
              return resp.fields
                .filter((f) => selectedColumns.has(f.field))
                .map((f) => f.field);
            })(),
            kind: AppColumnMenuKind.Redis,
            uuid,
          }}>
          <DataGrid
            columnVisibilityModel={getColumnVisibility(uuid, setKey)}
            onColumnVisibilityModelChange={(e) => {
              setColumnVisibility(uuid, setKey, e);
            }}
            getCellClassName={({ field }) => {
              if (selectedColumns.has(field)) {
                return "grid-column-cell-select ";
              }
            }}
            onCellClick={() => {
              setSelectColumns(new Set());
              allGridHeaderTitle(apiRef, (x) => {
                x.classList.remove(gridColumnHeaderSelectClassName);
              });
            }}
            apiRef={apiRef}
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
        </AppColumnMenuCtx>
      )}

      <ListenElementSize dom={modifyModalBox} set="both" name="modify-set-table" />

      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setSelectRow(null);
          })}
          disableAutoFocus
          disableEnforceFocus>
          <Box
            sx={{ resize: "both", ...modalStyle, width: "80%", height: "70vh" }}
            ref={setModifyModalBox}>
            <ModalHeader
              handleClose={() => {
                setSelectRow(null);
              }}
              title="Edit Set value"
            />
            <RedisEditSet
              modifyOP={async (oldValue, newValue) => {
                await modifyOP(oldValue, newValue);
                handleReload(false);
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

// function ListDelete(
//   props: RedisListTableContextMenuProps & { handleClose: () => void },
// ) {
//   const [count, setCount] = useState("1");
//   const { delOP, handleClose } = props;
//   const { showNotification } = useNotification();
//   return (
//     <Stack>
//       <TextField
//         value={count}
//         label="delete count"
//         onChange={(e) => {
//           setCount(e.target.value);
//         }}
//         helperText=<span
//           style={{ cursor: "pointer" }}
//           onClick={() => {
//             openUrl("https://redis.io/docs/latest/commands/lrem/");
//           }}>
//           count???
//         </span>
//         type="number"
//       />
//       <Stack direction="row">
//         <Button
//           onClick={async () => {
//             try {
//               await delOP(parseInt(count));
//               showNotification({
//                 title: "Delete OK",
//                 severity: "success",
//               });
//               handleClose();
//             } catch (e) {
//               showNotification({
//                 title: "delete failure",
//                 message: e,
//                 severity: "error",
//               });
//             }
//           }}
//           variant="outlined">
//           delete
//         </Button>
//       </Stack>
//     </Stack>
//   );
// }

function RedisListTableContextMenu(props: RedisListTableContextMenuProps) {
  const { content, delOP } = props;
  const { showNotification } = useNotification();
  return (
    <div>
      <MenuList>
        {content && (
          <MenuItem
            onClick={() => {
              handleCopyToClipboard(content, showNotification);
            }}>
            <ListItemIcon>
              <ContentCopyIcon />
            </ListItemIcon>
            <ListItemText> Copy </ListItemText>
          </MenuItem>
        )}
        <MenuItem
          onClick={async () => {
            try {
              await delOP(1);
              showNotification({
                title: "Delete OK",
                severity: "success",
              });
            } catch (e) {
              showNotification({
                title: "delete failure",
                message: e,
                severity: "error",
              });
            }
          }}>
          <ListItemIcon>
            <DeleteForeverIcon />
          </ListItemIcon>
          <ListItemText> Delete </ListItemText>
        </MenuItem>
      </MenuList>
    </div>
  );
}

type ListToolBarProps = {
  dbpal: ListToolBarDbpalProps;
} & GridSlotProps["toolbar"];

function ListToolBar({
  dbpal: { handleReload, listKey, loading, uuid },
}: ListToolBarProps) {
  return (
    <AppDataGridToolBar
      left=<>
        <RefreshButton
          color="primary"
          onClick={() => {
            handleReload(true);
          }}
          loading={loading}
        />
        <AddTableModal
          kind={DataSourceKind.Redis}
          uuid={uuid}
          tableName={listKey}
          redisType={RedisTableType.List}
        />
      </>
      title={listKey}
    />
  );
}
function RedisListTable(props: { listKey: string; uuid: string }) {
  const { listKey, uuid } = props;
  const modifyOP = async (index, value) => {
    return await commands.redisListSet(uuid, listKey, value, index);
  };
  const delOP = useCallback(
    async (count, value) => {
      return await commands.redisListRemove(uuid, listKey, value, count);
    },
    [listKey, uuid],
  );
  const apiRef = useGridApiRef();
  const [selectedColumns, setSelectColumns] = useState<Set<string>>(new Set());
  const [resp, setResp] = useState<MuiDataGridData>(newEmptyResp());
  const [respOrigin, setRespOrigin] = useState<MuiDataGridData>(newEmptyResp());
  const [selectRow, setSelectRow] = useState<any>(null);
  const { showNotification } = useNotification();
  const { setColumnWidth, getColumnWidth } = useColumnWidthStore();
  const { sql: sqlPageSize, setPageSize } = usePageSize();

  const [page, setPage] = useState(0);
  const [loading, setLoading] = useState(false);
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
              showNotification({ severity: "success", title: "reload OK" });
            }
          : undefined,

        handleResult: (resp) => {
          resp.data.forEach((e, index) => {
            e["id"] = index + "";
          });
          const seqWidth = getColumnWidth("seq");
          if (seqWidth) {
            resp.fields[0]["width"] = seqWidth;
          }
          resp.fields[1]["sortable"] = false;
          resp.fields[1]["flex"] = 1;
          resp.fields[1]["valueGetter"] = valueGetter;
          resp.fields.forEach((f) => {
            f["renderHeader"] = () => {
              return (
                <Stack
                  onClick={(event) => {
                    event.stopPropagation();
                    clickDataGridColumn(apiRef, event, f.field, setSelectColumns);
                  }}
                  style={{ ...columnHeaderStyle }}>
                  {f.field}
                </Stack>
              );
            };
          });
          setResp(resp);
          setRespOrigin(resp);
        },
        setStatus: setQueryStatus,
      });
    },
    [uuid, listKey, showNotification, getColumnWidth, apiRef],
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
      const del = resp.data.find((e) => e.id == id);
      try {
        await delOP(deleteCount, del.value);
        showNotification({ severity: "success", title: "Delete OK" });
      } catch (e) {
        showNotification({ severity: "error", title: "Delete failure", message: e });
      }
      handleReload(false);
    },
    [delOP, handleReload, resp, showNotification],
  );

  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);

  const onPaginationModelChange = useCallback(
    (e: GridPaginationModel, reason: GridCallbackDetails<"pagination">) => {
      setPage(e.page);
      if (reason.reason == "setPaginationModel") {
        setPageSize("sql", e.pageSize);
      }
    },
    [setPageSize],
  );

  const paginationModel = useMemo(() => {
    return { page, pageSize: sqlPageSize };
  }, [page, sqlPageSize]);

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
      });
    },
    [respOrigin],
  );

  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target } = e;
          const content = (target as HTMLDivElement).innerHTML;
          const { id, field } = extractFieldAndIDFromMouseEvent(e);
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
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
      toolbar: {
        dbpal: (function (): ListToolBarProps["dbpal"] {
          return { handleReload, listKey, loading, uuid };
        })(),
      },
    } as GridSlotsComponentsProps;
  }, [handleDel, handleReload, listKey, loading, setUpMenu, uuid]);

  const onColumnResize = useCallback(
    ({ colDef, width }) => {
      if (colDef.field == "seq") {
        setColumnWidth(colDef.field, width);
      }
    },
    [setColumnWidth],
  );

  const [modifyModalBox, setModifyModalBox] = useState<HTMLDivElement | null>(null);

  const slots = useMemo(() => {
    return {
      columnMenu: AppColumnMenu,
      toolbar: ListToolBar,
      pagination: RedisPagination,
    } as Partial<GridSlotsComponent>;
  }, []);
  const { getColumnVisibility, setColumnVisibility } = useTableColumnVisibleStore(
    (s) => s,
  );
  return (
    <Stack style={{ height: "100%" }}>
      <QueryStatus status={queryStatus} />
      {resp && (
        <AppColumnMenuCtx
          value={{
            selectColumns: (function () {
              return resp.fields
                .filter((f) => selectedColumns.has(f.field))
                .map((f) => f.field);
            })(),
            kind: AppColumnMenuKind.Redis,
            uuid,
          }}>
          <DataGrid
            getCellClassName={({ field }) => {
              if (selectedColumns.has(field)) {
                return "grid-column-cell-select ";
              }
            }}
            onCellClick={() => {
              setSelectColumns(new Set());
              allGridHeaderTitle(apiRef, (x) => {
                x.classList.remove(gridColumnHeaderSelectClassName);
              });
            }}
            apiRef={apiRef}
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
            columnVisibilityModel={getColumnVisibility(uuid, listKey)}
            onColumnVisibilityModelChange={(e) => {
              setColumnVisibility(uuid, listKey, e);
            }}
            onSortModelChange={onSortModelChange}
          />
        </AppColumnMenuCtx>
      )}

      <ListenElementSize dom={modifyModalBox} name="redis-modify-list" set="both" />

      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setSelectRow(null);
          })}
          disableAutoFocus
          disableEnforceFocus>
          <Box
            sx={{ resize: "both", ...modalStyle, width: "80%", height: "70vh" }}
            ref={setModifyModalBox}>
            <ModalHeader
              title="Edit List value"
              handleClose={() => {
                setSelectRow(null);
              }}
            />
            <RedisEditList
              modifyOP={async (newValue) => {
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

function RedisZSetTableContextMenu(props: RedisContextMenuProps) {
  const { delOP, content } = props;
  const { showNotification } = useNotification();
  return (
    <MenuList>
      {content && (
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}>
          <ListItemIcon>
            <ContentCopyIcon />
          </ListItemIcon>
          <ListItemText> Copy </ListItemText>
        </MenuItem>
      )}

      <MenuItem
        onClick={() => {
          delOP();
        }}>
        <ListItemIcon>
          <DeleteForeverIcon />
        </ListItemIcon>
        <ListItemText> Delete </ListItemText>
      </MenuItem>
    </MenuList>
  );
}

type ZSetToolBarProps = {
  dbpal: ZSetToolBarDbpalProps;
} & GridSlotProps["toolbar"];

function ZSetToolBar({
  dbpal: {
    keyMatch,
    minErr,
    max,
    min,
    maxErr,
    loading,
    handleReload,
    setKeyMatch,
    setMinErr,
    setMin,
    setMaxErr,
    setMax,
    uuid,
    zsetKey,
  },
}: ZSetToolBarProps) {
  return (
    <AppDataGridToolBar
      left=<>
        <RefreshButton
          color="primary"
          onClick={() => {
            handleReload(true);
          }}
          loading={loading}
        />

        <Tooltip title={keyMatcherHelpText}>
          <ClearableTextField
            label="key matcher"
            value={keyMatch}
            onChange={(event) => {
              setKeyMatch(event.target.value);
            }}
            onClear={() => setKeyMatch("")}
            style={{ width: "25%" }}
          />
        </Tooltip>

        <ClearableTextField
          label="minimum value"
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
          onClear={() => setMin("")}
          style={{ width: "15%" }}
        />
        <ClearableTextField
          label="maximum value"
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
          onClear={() => setMax("")}
          style={{ width: "15%" }}
        />
        <AddTableModal
          kind={DataSourceKind.Redis}
          uuid={uuid}
          tableName={zsetKey}
          redisType={RedisTableType.Zset}
        />
      </>
      title={zsetKey}
    />
  );
}

function RedisZsetTable(props: { uuid: string; zsetKey: string }) {
  const { uuid, zsetKey } = props;
  const modifyOP = async (oldMember: string, newKey: string, newValue: number) => {
    return await commands.redisZsetReplace(uuid, zsetKey, oldMember, newKey, newValue);
  };

  const apiRef = useGridApiRef();
  const [selectedColumns, setSelectColumns] = useState<Set<string>>(new Set());
  const delOP = useCallback(
    async (member) => {
      return await commands.redisZsetDel(uuid, zsetKey, member);
    },
    [uuid, zsetKey],
  );
  const { setColumnWidth, getColumnWidth } = useColumnWidthStore();
  const [keyMatch, setKeyMatch] = useState("");
  const [keyMatchDelay] = useDebounce(keyMatch, 1000);

  const [resp, setResp] = useState<MuiDataGridData>(newEmptyResp());
  const [selectRow, setSelectRow] = useState<any>();
  const [min, setMin] = useState("");
  const [max, setMax] = useState("");
  const [minDelay] = useDebounce(min, 1000);
  const [maxDelay] = useDebounce(max, 1000);
  const [minErr, setMinErr] = useState(false);
  const [maxErr, setMaxErr] = useState(false);
  const [loading, setLoading] = useState(false);
  const { showNotification } = useNotification();

  const getSize = useSizeStore((s) => s.getSize);

  const [queryStatus, setQueryStatus] = useState("");
  const handleReload = useCallback(
    (showOK: boolean) => {
      if (minErr || maxErr) {
        return;
      }
      const minValue = parseFloat(minDelay);
      const maxValue = parseFloat(maxDelay);
      setLoading(true);
      showTable({
        setStatus: setQueryStatus,
        uuid,
        name: zsetKey,
        setLoading,
        para: {
          min: minValue,
          max: maxValue,
          where_str: keyMatchDelay ? keyMatchDelay : null,
          limit: getSize(UseSizeConfig.queryLimit),
        },

        showNotification,
        okCallBack: showOK
          ? () => {
              showNotification({ severity: "success", title: "reload OK" });
            }
          : undefined,
        handleResult: (resp) => {
          resp.data.forEach((e, index) => {
            e["id"] = index + "";
          });
          resp.fields[1]["flex"] = 1;
          const scoreWidth = getColumnWidth("score");
          if (scoreWidth) {
            resp.fields[0]["width"] = scoreWidth;
          }

          resp.fields[1]["valueGetter"] = valueGetter;
          resp.fields.forEach((f) => {
            f["renderHeader"] = () => {
              return (
                <Stack
                  onClick={(event) => {
                    event.stopPropagation();
                    clickDataGridColumn(apiRef, event, f.field, setSelectColumns);
                  }}
                  style={{ ...columnHeaderStyle }}>
                  {f.field}
                </Stack>
              );
            };
          });
          setResp(resp);
        },
      });
    },
    [
      minErr,
      maxErr,
      minDelay,
      maxDelay,
      uuid,
      zsetKey,
      keyMatchDelay,
      getSize,
      showNotification,
      getColumnWidth,
      apiRef,
    ],
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
      const del = resp.data.find((e) => e["id"] == id);
      try {
        await delOP(del.key);
        showNotification({ severity: "success", title: "Delete OK" });
      } catch (e) {
        showNotification({ severity: "error", title: "delete failure", message: e });
      }
      handleReload(false);
    },
    [delOP, handleReload, resp, showNotification],
  );
  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);

  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
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
      pagination: { dbpal: { showLimit: true } },
      toolbar: {
        dbpal: (function (): ZSetToolBarProps["dbpal"] {
          return {
            min,
            max,
            minErr,
            maxErr,
            setMin,
            setMax,
            setMinErr,
            setMaxErr,
            setKeyMatch,
            uuid,
            zsetKey,
            keyMatch,
            loading,
            handleReload,
          };
        })(),
      },
    } as GridSlotsComponentsProps;
  }, [
    handleDel,
    handleReload,
    keyMatch,
    loading,
    max,
    maxErr,
    min,
    minErr,
    setUpMenu,
    uuid,
    zsetKey,
  ]);

  const [modifyModalBox, setModifyModalBox] = useState<HTMLDivElement | null>(null);

  const onColumnResize = useCallback(
    ({ colDef, width }) => {
      if (colDef.field == "score") {
        setColumnWidth(colDef.field, width);
      }
    },
    [setColumnWidth],
  );

  const slots = useMemo((): Partial<GridSlotsComponent> => {
    return {
      columnMenu: AppColumnMenu,
      toolbar: ZSetToolBar,
      pagination: RedisPagination,
    };
  }, []);

  const { getColumnVisibility, setColumnVisibility } = useTableColumnVisibleStore(
    (s) => s,
  );
  return (
    <Stack style={{ height: "100%" }}>
      <QueryStatus status={queryStatus} />

      <ListenElementSize dom={modifyModalBox} name="redis-modify-zset" set="both" />

      {resp && (
        <AppColumnMenuCtx
          value={{
            selectColumns: (function () {
              return resp.fields
                .filter((f) => selectedColumns.has(f.field))
                .map((f) => f.field);
            })(),
            kind: AppColumnMenuKind.Redis,
            uuid,
          }}>
          <DataGrid
            columnVisibilityModel={getColumnVisibility(uuid, zsetKey)}
            onColumnVisibilityModelChange={(e) => {
              setColumnVisibility(uuid, zsetKey, e);
            }}
            getCellClassName={({ field }) => {
              if (selectedColumns.has(field)) {
                return "grid-column-cell-select ";
              }
            }}
            onCellClick={() => {
              setSelectColumns(new Set());
              allGridHeaderTitle(apiRef, (x) => {
                x.classList.remove(gridColumnHeaderSelectClassName);
              });
            }}
            apiRef={apiRef}
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
        </AppColumnMenuCtx>
      )}
      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setSelectRow(null);
          })}
          disableAutoFocus
          disableEnforceFocus>
          <Box
            sx={{ resize: "both", ...modalStyle, width: "80%", height: "70vh" }}
            ref={setModifyModalBox}>
            <ModalHeader
              title="Edit Zset Value"
              handleClose={() => {
                setSelectRow(null);
              }}
            />
            <RedisEditZset
              modifyOP={async (oldKey, newKey, value) => {
                await modifyOP(oldKey, newKey, value);
                handleReload(false);
              }}
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

function RedisPagination(props: { dbpal?: { showLimit: boolean } }) {
  const showLimit = props.dbpal?.showLimit ?? false;

  return (
    <Stack
      direction="row"
      sx={{
        alignItems: "center",
        justifyContent: "space-between",
        flex: 1,
        px: 1,
      }}>
      <StackRowCenter>
        <FetchTimeDataGrid />
        {showLimit && <SliderSize config={UseSizeConfig.queryLimit} />}
        <ResetColumnWidth />
        <DoubleClickToModify />
      </StackRowCenter>

      <Stack direction="row" sx={{ alignItems: "center" }}>
        <AppDataGridPagination showPageSelect />
      </Stack>
    </Stack>
  );
}

function RedisHashTableContextMenu(props: RedisContextMenuProps) {
  const { delOP, content } = props;

  const { showNotification } = useNotification();
  return (
    <MenuList>
      {content && (
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}>
          <ListItemIcon>
            <ContentCopyIcon />
          </ListItemIcon>
          <ListItemText> Copy </ListItemText>
        </MenuItem>
      )}

      <MenuItem
        onClick={() => {
          delOP();
        }}>
        <ListItemIcon>
          <DeleteForeverIcon />
        </ListItemIcon>
        <ListItemText> Delete </ListItemText>
      </MenuItem>
    </MenuList>
  );
}

const valueGetter = (value) => {
  const x = hexTryToString(value);
  if (typeof x === "string") {
    return x;
  } else {
    return "binary value of " + value;
  }
};

type HashToolBarProps = {
  dbpal: HashToolBarDbpalProps;
} & GridSlotProps["toolbar"];

function HashToolBar({
  dbpal: { keyMatch, setKeyMatch, uuid, hashKey, loading, handleReload },
}: HashToolBarProps) {
  return (
    <AppDataGridToolBar
      left=<>
        <RefreshButton
          color="primary"
          onClick={() => {
            handleReload(true);
          }} // 点击事件处理
          loading={loading}
        />

        <Tooltip title={keyMatcherHelpText}>
          <ClearableTextField
            label="key matcher"
            value={keyMatch}
            onChange={(event) => {
              setKeyMatch(event.target.value);
            }}
            onClear={() => setKeyMatch("")}
            style={{ maxWidth: "15rem", flex: 1 }}
          />
        </Tooltip>
        <AddTableModal
          kind={DataSourceKind.Redis}
          uuid={uuid}
          tableName={hashKey}
          redisType={RedisTableType.Hash}
        />
      </>
      title={hashKey}
    />
  );
}

function RedisHashTable(props: { hashKey: string; uuid: string }) {
  const { uuid, hashKey } = props;
  const apiRef = useGridApiRef();

  const delOP = useCallback(
    async (member) => {
      return await commands.redisHashDel(uuid, hashKey, member);
    },
    [hashKey, uuid],
  );
  const [selectedColumns, setSelectColumns] = useState<Set<string>>(new Set());

  const modifyOP = async (oldMember, newMember, newValue) => {
    return await commands.redisHashReplace(
      uuid,
      hashKey,
      oldMember,
      newMember,
      newValue,
    );
  };
  const { getColumnWidth, setColumnWidth } = useColumnWidthStore();

  const [keyMatch, setKeyMatch] = useState("");
  const [keyMatchDelay] = useDebounce(keyMatch, 1000);
  const [resp, setResp] = useState<MuiDataGridData>(newEmptyResp());
  const { showNotification } = useNotification();
  const [selectRow, setSelectRow] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const getSize = useSizeStore((s) => s.getSize);
  const [queryStatus, setQueryStatus] = useState("");

  const handleReload = useCallback(
    (showOK) => {
      showTable({
        uuid,
        name: hashKey,
        okCallBack: showOK
          ? () => {
              showNotification({ severity: "success", title: "reload OK" });
            }
          : undefined,

        setStatus: setQueryStatus,
        para: {
          limit: getSize(UseSizeConfig.queryLimit),
          where_str: keyMatchDelay ? keyMatchDelay : null,
        },
        setLoading,
        showNotification,
        handleResult: (resp) => {
          resp.data.forEach((e, index) => {
            e["id"] = index + "";
          });
          const keyWidth = getColumnWidth("key");
          resp.fields[0]["width"] = keyWidth;
          resp.fields[1]["flex"] = 1;
          resp.fields[0]["valueGetter"] = valueGetter;
          resp.fields[1]["valueGetter"] = valueGetter;

          resp.fields.forEach((f) => {
            f["renderHeader"] = () => {
              return (
                <Stack
                  onClick={(event) => {
                    event.stopPropagation();
                    clickDataGridColumn(apiRef, event, f.field, setSelectColumns);
                  }}
                  style={{ ...columnHeaderStyle }}>
                  {f.field}
                </Stack>
              );
            };
          });
          setResp(resp);
        },
      });
    },
    [apiRef, getColumnWidth, getSize, hashKey, keyMatchDelay, showNotification, uuid],
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
      const del = resp.data.find((e) => e["id"] == id);
      try {
        await delOP(del.key);
        showNotification({ severity: "success", title: "Delete OK" });
      } catch (e) {
        showNotification({ severity: "error", title: "Delete failure", message: e });
      }

      handleReload(false);
    },
    [delOP, handleReload, resp, showNotification],
  );
  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);

  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
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
      pagination: { dbpal: { showLimit: true } },
      toolbar: {
        dbpal: (function (): HashToolBarProps["dbpal"] {
          return { keyMatch, setKeyMatch, uuid, loading, handleReload, hashKey };
        })(),
      },
    } as GridSlotsComponentsProps;
  }, [handleDel, handleReload, hashKey, keyMatch, loading, setUpMenu, uuid]);

  const [modifyModalBox, setModifyModalBox] = useState<HTMLDivElement | null>(null);

  const onColumnResize = useCallback(
    ({ colDef, width }) => {
      if (colDef.field == "key") {
        setColumnWidth(colDef.field, width);
      }
    },
    [setColumnWidth],
  );
  const { getColumnVisibility, setColumnVisibility } = useTableColumnVisibleStore(
    (s) => s,
  );
  const slots = useMemo(() => {
    return {
      columnMenu: AppColumnMenu,
      toolbar: HashToolBar,
      pagination: RedisPagination,
    } as Partial<GridSlotsComponent>;
  }, []);

  return (
    <Stack style={{ height: "100%" }}>
      <QueryStatus status={queryStatus} />
      <ListenElementSize dom={modifyModalBox} name="redis-modify-hash" set="both" />
      {resp && (
        <AppColumnMenuCtx
          value={{
            selectColumns: (function () {
              return resp.fields
                .filter((f) => selectedColumns.has(f.field))
                .map((f) => f.field);
            })(),
            kind: AppColumnMenuKind.Redis,
            uuid,
          }}>
          <DataGrid
            columnVisibilityModel={getColumnVisibility(uuid, hashKey)}
            onColumnVisibilityModelChange={(e) => {
              setColumnVisibility(uuid, hashKey, e);
            }}
            getCellClassName={({ field }) => {
              if (selectedColumns.has(field)) {
                return "grid-column-cell-select ";
              }
            }}
            onCellClick={() => {
              setSelectColumns(new Set());
              allGridHeaderTitle(apiRef, (x) => {
                x.classList.remove(gridColumnHeaderSelectClassName);
              });
            }}
            apiRef={apiRef}
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
        </AppColumnMenuCtx>
      )}

      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setSelectRow(null);
          })}
          disableAutoFocus
          disableEnforceFocus>
          <Box
            sx={{ resize: "both", ...modalStyle, width: "80%", height: "70vh" }}
            ref={setModifyModalBox}>
            <ModalHeader
              title="Edit Hash Value"
              handleClose={() => {
                setSelectRow(null);
              }}
            />
            <RedisEditHash
              modifyOP={async (req) => {
                await modifyOP(req.oldKey, req.newKey, req.value);
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
