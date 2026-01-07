import { ListItemIcon, ListItemText, MenuItem, MenuList, Stack } from "@mui/material";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import { useCallback, useContext, useEffect, useRef, useState } from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import InfoIcon from "@mui/icons-material/Info";
import { DefAndMetas, tabsHasId } from "./Common";
import types from "./ApiStruct";
import { invoke } from "@tauri-apps/api/core";
import { NotificationContext } from "./Notification";
import { TabContent } from "./Tabs";
import { DefAndMetasView } from "./UdfAndMetaView";
import { PiTreeStructureFill } from "react-icons/pi";
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
import { ExceedMaxTreeItem, MenuRefresh } from "./CommonComp";
import { listCreatedTypes } from "./handlers";
function TypesTree() {
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const [types, setTypes] = useState<types.CreatedTypeDef[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const maxsize = useContext(MaxTreeContext);

  const reload = useCallback(() => {
    listCreatedTypes(uuid)
      .then(ts => {
        setTypes(ts);
      })
      .catch(e => {
        showNotification("error", "list types failed", e);
      });
  }, [showNotification, uuid]);
  const [exceed, setExceed] = useState(0);
  const matchPattern = useContext(MatchPatternContext);
  useEffect(() => {
    setExceed(types.filter(e => matchPattern(e.name)).length);
  }, [types, matchPattern]);

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
        <PiTreeStructureFill />
        <span> types </span>
      </Stack>
    >
      {types
        .filter(typ => matchPattern(typ.name))
        .map((typ, index) => {
          if (index >= maxsize) {
            return;
          }
          const id = `${uuid}-type-${index}`;
          return (
            <TreeItem
              key={typ.name}
              itemId={id}
              style={{ alignItems: "center" }}
              label=<Stack
                direction="row"
                onContextMenu={e => {
                  setUpMenu({
                    content: (
                      <TypeMenu
                        reload={reload}
                        uuid={uuid}
                        kind={kind}
                        createdType={typ}
                        id={id}
                      />
                    ),
                    event: e,
                  });
                }}
                sx={{ alignItems: "center" }}
              >
                <PiTreeStructureFill />
                <span> {typ.name} </span>
              </Stack>
            />
          );
        })}
      <ExceedMaxTreeItem exceed={exceed} maxTreeSize={maxsize} />
    </TreeItem>
  );
}

function TypeMenu(props: {
  uuid;
  createdType: types.CreatedTypeDef;
  reload;
  kind: DataSourceKind;
  id: string;
}) {
  const { uuid, createdType, reload, kind, id } = props;
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
            createdType.name,
            (
              <DefAndMetasView
                reloadOP={async () => {
                  return Promise.resolve({
                    def: createdType.sql,
                  } as DefAndMetas);
                }}
                kind={kind}
                uuid={uuid}
              />
            ),
            <PiTreeStructureFill />,
          ),
        ];
      });
    }
  };
  const handleDrop = async () => {
    const confirm = await secondConfirm({
      title: `Drop type '${createdType.name}'`,
      confirmText: "Drop",
      kind: "error",
    });
    if (!confirm) {
      return;
    }

    invoke("drop_created_type", { name: uuid, typeName: createdType.name })
      .then(() => {
        showNotification("success", "Drop type OK");
        reload();
      })

      .catch(e => {
        showNotification("error", "Drop type Failure", e);
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

export { TypesTree };
