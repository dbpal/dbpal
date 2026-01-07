import { ListItemIcon, ListItemText, MenuItem, MenuList, Stack } from "@mui/material";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import { useCallback, useContext, useEffect, useRef, useState } from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import InfoIcon from "@mui/icons-material/Info";
import { tabsHasId } from "./Common";
import types from "./ApiStruct";
import { invoke } from "@tauri-apps/api/core";
import { NotificationContext } from "./Notification";
import { TabContent } from "./Tabs";
import { DefAndMetasView } from "./UdfAndMetaView";

import { SiProcessingfoundation } from "react-icons/si";
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
} from "./helper";
import { ExceedMaxTreeItem, MenuRefresh, UdfNameAndParameters } from "./CommonComp";
import { listProcedure, makeShowProcedureExtraArg, showProcedure } from "./handlers";
function ProceduresTree() {
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const [ps, setPos] = useState<types.NamedStaff[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const maxsize = useContext(MaxTreeContext);
  const reload = useCallback(() => {
    listProcedure(uuid)
      .then(ts => {
        setPos(ts);
      })
      .catch(e => {
        showNotification("error", "list procedure failed", e);
      });
  }, [showNotification, uuid]);

  const [exceed, setExceed] = useState(0);
  const matchPattern = useContext(MatchPatternContext);
  useEffect(() => {
    setExceed(ps.filter(e => matchPattern(e.name)).length);
  }, [ps, matchPattern]);

  const { setUpMenu } = useContext(ContextMenuContext);
  useEffect(() => {
    reload();
  }, [reload]);

  const idForTrees = useRef(generateRandomString());
  return (
    <TreeItem
      itemId={idForTrees.current}
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
        <SiProcessingfoundation />
        <span> procedures </span>
      </Stack>
    >
      {ps
        .filter(e => matchPattern(e.name))
        .map((p, index) => {
          if (index >= maxsize) {
            return;
          }
          const id = `${uuid}-procedure-${index}`;
          return (
            <TreeItem
              key={p.name + "-" + p.parameters}
              itemId={id}
              style={{ alignItems: "center" }}
              label=<Stack
                direction="row"
                onContextMenu={e => {
                  setUpMenu({
                    content: (
                      <ProcedureMenu
                        reload={reload}
                        uuid={uuid}
                        kind={kind}
                        procedure={p}
                        id={id}
                      />
                    ),
                    event: e,
                  });
                }}
                sx={{ alignItems: "center" }}
              >
                <SiProcessingfoundation />
                <UdfNameAndParameters udf={p} kind={kind} />
              </Stack>
            />
          );
        })}
      <ExceedMaxTreeItem maxTreeSize={maxsize} exceed={exceed} />
    </TreeItem>
  );
}

function ProcedureMenu(props: {
  uuid: string;
  procedure: types.NamedStaff;
  reload: () => void;
  kind: DataSourceKind;
  id: string;
}) {
  const { uuid, procedure, reload, kind, id } = props;
  const { setSelectTab, setTabs, tabs } = useContext(TabsSetContext);
  const { showNotification } = useContext(NotificationContext);

  const handleOpenDetails = () => {
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
            procedure.name,
            (
              <DefAndMetasView
                showReload
                reloadOP={async () => {
                  return showProcedure(uuid, procedure, kind);
                }}
                kind={kind}
                uuid={uuid}
              />
            ),
            <SiProcessingfoundation />,
          ),
        ];
      });
    }
  };

  const handleDrop = async () => {
    const confirm = await secondConfirm({
      title: `Drop Procedure '${procedure}'`,
      confirmText: "Drop",
      kind: "error",
    });
    if (!confirm) {
      return;
    }
    const arg = makeShowProcedureExtraArg(procedure, kind);
    invoke("drop_procedure", { name: uuid, procedure, arg })
      .then(() => {
        showNotification("success", "Drop procedure OK");
        reload();
      })
      .catch(e => {
        showNotification("error", "Drop procedure Failure", e);
      });
  };
  return (
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
          handleDrop();
        }}
      >
        <ListItemIcon>
          <DeleteForeverIcon color="error" />
        </ListItemIcon>
        <ListItemText> Drop</ListItemText>
      </MenuItem>
    </MenuList>
  );
}

export { ProceduresTree };
