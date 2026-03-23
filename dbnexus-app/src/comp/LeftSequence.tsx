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
} from "./Common.ts";
import types from "./ApiStruct.ts";
import { invoke } from "@tauri-apps/api/core";
import { NotificationContext } from "./Notification.tsx";
import { TabContent } from "./Tabs.tsx";
import { DefAndMetasView } from "./DefAndMetaView.tsx";
import {
  ConnTypeContext,
  ContextMenuContext,
  DataSourceKind,
  generateRandomString,
  MatchPatternContext,
  MaxTreeContext,
  secondConfirm,
  TabsSetContext,
  UuidContext,
} from "./helper.ts";

import { ExceedMaxTreeItem, MenuRefresh } from "./CommonComp.tsx";
import { listSequence } from "./handlers.ts";
import { GrSteps } from "react-icons/gr";
import { CopySql } from "./CommonComp2.tsx";
function SequenceTree() {
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const [seq, setSeq] = useState<types.SequenceDetail[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const maxsize = useContext(MaxTreeContext);
  const reload = useCallback(() => {
    listSequence(uuid)
      .then((ts) => {
        setSeq(ts);
      })
      .catch((e) => {
        showNotification("error", "list sequence failed", e);
      });
  }, [showNotification, uuid]);

  const [exceed, setExceed] = useState(0);
  const matchPattern = useContext(MatchPatternContext);
  useEffect(() => {
    setExceed(seq.filter((e) => matchPattern(e.name)).length);
  }, [seq, matchPattern]);
  const { setUpMenu } = useContext(ContextMenuContext);
  useEffect(() => {
    reload();
  }, [reload]);
  const idForTree = useRef(generateRandomString());
  return (
    <TreeItem
      itemId={idForTree.current}
      label={
        <Stack
          direction="row"
          sx={{ alignItems: "center" }}
          onContextMenu={(e) => {
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
          <GrSteps />
          <span> sequences </span>
        </Stack>
      }
    >
      {seq
        .filter((e) => matchPattern(e.name))
        .map((seq, index) => {
          if (index >= maxsize) {
            return;
          }
          const id = `${uuid}-sequence-${seq.name}`;
          return (
            <TreeItem
              key={seq.name}
              itemId={id}
              style={{ alignItems: "center" }}
              label={
                <Stack
                  direction="row"
                  onContextMenu={(e) => {
                    setUpMenu({
                      content: (
                        <SequenceMenu
                          reload={reload}
                          uuid={uuid}
                          kind={kind}
                          seq={seq}
                          id={id}
                        />
                      ),
                      event: e,
                    });
                  }}
                  sx={{ alignItems: "center" }}
                >
                  <GrSteps />
                  <span>{seq.name}</span>
                </Stack>
              }
            />
          );
        })}
      <ExceedMaxTreeItem exceed={exceed} maxTreeSize={maxsize} />
    </TreeItem>
  );
}

function details2DefMetas(d: types.SequenceDetail): DefAndMetas {
  return {
    def: d.sql,
    metas: [
      { name: "data type", value: d.data_type },
      { name: "start_value", value: d.start_value },
      { name: "min value", value: d.min_value },
      { name: "max value", value: d.max_value },
      { name: "increment by", value: d.increment_by },
      { name: "last value", value: d.last_value },
    ],
  };
}

function SequenceMenu(props: {
  uuid;
  seq: types.SequenceDetail;
  reload;
  kind: DataSourceKind;
  id: string;
}) {
  const { uuid, seq, reload, kind, id } = props;
  const { setSelectTab, tabs, appendTab } = useContext(TabsSetContext);
  const { showNotification } = useContext(NotificationContext);
  const [showDrop, setShowDrop] = useState(false);
  const [cascade, setCascade] = useState(false);
  const supportCascade = [DataSourceKind.Postgres].includes(kind);

  const handleOpenDetails = () => {
    const index = tabsHasId(tabs, id);
    if (typeof index == "number") {
      setSelectTab(index);
    } else {
      appendTab(
        new TabContent(
          id,
          seq.name,
          <DefAndMetasView
            reloadOP={() => {
              return Promise.resolve(details2DefMetas(seq));
            }}
            kind={kind}
            uuid={uuid}
          />,
          <GrSteps className="tab-icon-size" />,
        ),
      );
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
      .then((sql) => {
        showNotification("success", "Drop sequence OK", <CopySql sql={sql} />);
        reload();
        setShowDrop(false);
      })
      .catch((e) => {
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
                        onChange={(e) => setCascade(e.target.checked)}
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
