"use strict";
import {
  Box,
  Button,
  Checkbox,
  FormControlLabel,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Stack,
} from "@mui/material";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import { useCallback, useContext, useEffect, useRef, useState } from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import InfoIcon from "@mui/icons-material/Info";
import {
  DefAndMetas,
  EscDownEvent,
  modalHandleClose,
  modalStyle,
  tabsHasId,
} from "./Common";
import types from "./ApiStruct";
import { invoke } from "@tauri-apps/api/core";
import { NotificationContext } from "./Notification";
import { TabContent } from "./Tabs";
import { DefAndMetasView } from "./UdfAndMetaView";
import { GiProgression } from "react-icons/gi";
import {
  ConnTypeContext,
  ContextMenuContext,
  DataSourceKind,
  generateRandomString,
  MatchPatternContext,
  secondConfirm,
  SettingContext,
  TabsSetContext,
  UuidContext,
} from "./helper";

import { ExceedMaxTreeItem, MenuRefresh } from "./CommonComp";
import { listSequence } from "./handlers";
function SequenceTree() {
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const idPrefix = uuid + "-sequence-";
  const [seq, setSeq] = useState<types.SequenceDetail[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const setting = useContext(SettingContext);

  const reload = useCallback(() => {
    listSequence(uuid)
      .then(ts => {
        setSeq(ts);
      })
      .catch(e => {
        showNotification("error", "list sequence failed", e);
      });
  }, [showNotification, uuid]);

  const [exceed, setExceed] = useState(0);
  const matchPattern = useContext(MatchPatternContext);
  useEffect(() => {
    setExceed(seq.filter(e => matchPattern(e.name)).length);
  }, [seq, matchPattern]);
  const getItemId = (x: types.SequenceDetail) => {
    switch (kind) {
      default:
        return idPrefix + "-" + x.name;
    }
  };

  const { setUpMenu } = useContext(ContextMenuContext);
  useEffect(() => {
    reload();
  }, [reload]);
  const idForTree = useRef(generateRandomString());
  return (
    <TreeItem
      itemId={idForTree.current}
      label=<Stack
        direction="row"
        sx={{ alignItems: "center" }}
        onContextMenu={e => {
          setUpMenu({
            content: (
              <MenuList>
                <MenuRefresh reload={reload} />
              </MenuList>
            ),
            event: e,
          });
        }}
      >
        <GiProgression />
        <span> sequences </span>
      </Stack>
    >
      {seq
        .filter(e => matchPattern(e.name))
        .map((seq, index) => {
          if (index >= setting.appearance.maxTreeSize) {
            return;
          }
          return (
            <TreeItem
              key={seq.name}
              itemId={getItemId(seq)}
              style={{ alignItems: "center" }}
              label=<Stack
                direction="row"
                onContextMenu={e => {
                  setUpMenu({
                    content: (
                      <SequenceMenu reload={reload} uuid={uuid} kind={kind} seq={seq} />
                    ),
                    event: e,
                  });
                }}
                sx={{ alignItems: "center" }}
              >
                <GiProgression />
                <span>{seq.name}</span>
              </Stack>
            />
          );
        })}
      <ExceedMaxTreeItem exceed={exceed} maxTreeSize={setting.appearance.maxTreeSize} />
    </TreeItem>
  );
}

function details2DefMetas(d: types.SequenceDetail): DefAndMetas {
  return {
    def: d.sql,
    metas: [
      { name: "data type", value: d.data_type, desc: null },
      { name: "start_value", value: d.start_value, desc: null },
      { name: "min value", value: d.min_value, desc: null },
      { name: "max value", value: d.max_value, desc: null },
      { name: "increment by", value: d.increment_by, desc: null },
      { name: "last value", value: d.last_value, desc: null },
    ],
  };
}

function SequenceMenu(props: {
  uuid;
  seq: types.SequenceDetail;
  reload;
  kind: DataSourceKind;
}) {
  const { uuid, seq, reload, kind } = props;
  const { setSelectTab, setTabs, tabs } = useContext(TabsSetContext);
  const { showNotification } = useContext(NotificationContext);
  const [showDrop, setShowDrop] = useState(false);
  const [cascade, setCascade] = useState(false);
  const supportCascade = [DataSourceKind.Postgres].includes(kind);

  const handleOpenDetails = () => {
    const id = uuid + "-" + "-sequence-" + seq.name;
    const index = tabsHasId(tabs, id);
    if (typeof index == "number") {
      setSelectTab(index);
    } else {
      setTabs(e => {
        setTimeout(() => {
          setSelectTab(tabs.length);
        }, 100);

        return [
          ...e,
          new TabContent(
            id,
            seq.name,
            (
              <DefAndMetasView
                reloadOP={async () => {
                  return Promise.resolve(details2DefMetas(seq));
                }}
                kind={kind}
                uuid={uuid}
              />
            ),
            <GiProgression />,
          ),
        ];
      });
    }
  };

  const handleDrop = async () => {
    const confirm = await secondConfirm({
      title: `Drop Sequence '${seq.name}'`,
      confirmText: "Drop",
      kind: "error",
    });
    if (!confirm) {
      return;
    }
    invoke("drop_sequence", { name: uuid, seq: seq.name, cascade })
      .then(() => {
        showNotification("success", "Drop sequence OK");
        reload();
        setShowDrop(false);
      })
      .catch(e => {
        showNotification("error", "Drop sequence Failure", e);
      });
  };
  return (
    <div>
      <MenuList>
        <MenuItem
          onClick={() => {
            handleOpenDetails();
          }}
        >
          <ListItemIcon>
            <InfoIcon />
          </ListItemIcon>
          <ListItemText> Detail </ListItemText>
        </MenuItem>

        <MenuItem
          sx={{ color: "error.main" }}
          onClick={() => {
            setShowDrop(true);
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon color="error" />
          </ListItemIcon>
          <ListItemText> Drop</ListItemText>
        </MenuItem>
      </MenuList>
      {showDrop && (
        <Modal
          open={showDrop}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowDrop(false);
          })}
        >
          <Box sx={modalStyle} width="45%">
            <Stack>
              {supportCascade && (
                <Stack>
                  <FormControlLabel
                    control={
                      <Checkbox
                        checked={cascade}
                        onChange={e => setCascade(e.target.checked)}
                      />
                    }
                    label="CasCade"
                  />
                </Stack>
              )}
              <Stack direction="row">
                <Button
                  onClick={() => {
                    handleDrop();
                  }}
                  color="error"
                >
                  Drop
                </Button>
              </Stack>
            </Stack>
          </Box>
        </Modal>
      )}
    </div>
  );
}

export { SequenceTree };
