import { ListItemIcon, ListItemText, MenuItem, MenuList, Stack } from "@mui/material";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import { useCallback, useContext, useEffect, useRef, useState } from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import InfoIcon from "@mui/icons-material/Info";
import { tabsHasId } from "./Common";
import types from "./ApiStruct";
import { invoke } from "@tauri-apps/api/core";
import { DefAndMetasView } from "./DefAndMetaView";
import { PiTreeStructureFill } from "react-icons/pi";
import {
  DataSourceKind,
  generateRandomString,
  secondConfirm,
  TabContent,
  textEllipsisStyle,
  UuidContext,
} from "./helper";
import { ExceedMaxTreeItem, MenuRefresh } from "./CommonComp";
import { listCreatedTypes } from "./handlers";
import { CopySql } from "./CommonComp2";
import {
  useMatcher,
  useContextMenu,
  useNotification,
  useTabsManage,
  useMatcherFn,
} from "./store";
function TypesTree() {
  const { uuid, kind } = useContext(UuidContext);
  const [types, setTypes] = useState<types.CreatedTypeDef[]>([]);
  const { showNotification } = useNotification();
  const { maxTree } = useMatcher((state) => state);
  const matchPattern = useMatcherFn();

  const reload = useCallback(() => {
    listCreatedTypes(uuid)
      .then((ts) => {
        setTypes(ts);
      })
      .catch((e) => {
        showNotification("error", "list types failed", e);
      });
  }, [showNotification, uuid]);
  const [exceed, setExceed] = useState(0);
  useEffect(() => {
    setExceed(types.filter((e) => matchPattern(e.name)).length);
  }, [types, matchPattern]);

  const setUpMenu = useContextMenu((state) => state.setUpMenu);
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
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <MenuList>
                <MenuRefresh reload={reload} />
              </MenuList>
            ),
            event: e,
          });
        }}>
        <PiTreeStructureFill />
        <span> types </span>
      </Stack>>
      {types
        .filter((typ) => matchPattern(typ.name))
        .map((typ, index) => {
          if (index >= maxTree) {
            return;
          }
          const id = `${uuid}-type-${typ.name}`;
          return (
            <TreeItem
              key={typ.name}
              itemId={id}
              style={{ alignItems: "center" }}
              label=<Stack
                direction="row"
                onContextMenu={(e) => {
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
                sx={{ alignItems: "center" }}>
                <PiTreeStructureFill />
                <span style={textEllipsisStyle}> {typ.name} </span>
              </Stack>
            />
          );
        })}
      <ExceedMaxTreeItem exceed={exceed} maxTreeSize={maxTree} />
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
  const { setCurrentTab, tabs, appendTab } = useTabsManage((state) => state);
  const { showNotification } = useNotification();
  const handleOpenDetails = () => {
    const index = tabsHasId(tabs, id);
    if (typeof index == "number") {
      setCurrentTab(index);
    } else {
      appendTab(
        new TabContent(
          id,
          createdType.name,
          <DefAndMetasView
            reloadOP={async () => {
              return Promise.resolve({
                def: createdType.sql,
                metas: [],
              });
            }}
            kind={kind}
            uuid={uuid}
          />,
          <PiTreeStructureFill />,
        ),
      );
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
      .then((sql) => {
        showNotification("success", "Drop type OK", <CopySql sql={sql} />);
        reload();
      })

      .catch((e) => {
        showNotification("error", "Drop type Failure", e);
      });
  };
  return (
    <MenuList>
      <MenuItem
        onClick={() => {
          handleOpenDetails();
        }}>
        <ListItemIcon>
          <InfoIcon />
        </ListItemIcon>
        <ListItemText> Detail </ListItemText>
      </MenuItem>

      <MenuItem
        sx={{ color: "error.main" }}
        onClick={() => {
          handleDrop();
        }}>
        <ListItemIcon>
          <DeleteForeverIcon color="error" />
        </ListItemIcon>
        <ListItemText> Drop</ListItemText>
      </MenuItem>
    </MenuList>
  );
}

export { TypesTree };
