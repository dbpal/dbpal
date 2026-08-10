import { ListItemIcon, ListItemText, MenuItem, MenuList, Stack } from "@mui/material";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import { useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import InfoIcon from "@mui/icons-material/Info";
import { DefAndMetasView } from "./DefAndMetaView";
import { PiTreeStructureFill } from "react-icons/pi";
import {
  DataSourceKind,
  secondConfirm,
  TabContent,
  textEllipsisStyle,
  UuidContext,
} from "./helper";
import { MenuRefresh } from "./CommonComp";
import { CopySql, ShowingCount } from "./CommonComp2";
import {
  useMatcher,
  useContextMenu,
  useNotification,
  useTabsManage,
  useMatcherFn,
} from "./store";
import { commands, CreatedTypeDef } from "./bindings";
import { uuidv4 } from "./Common";
function TypesTree() {
  const { dbName, kind } = useContext(UuidContext);
  const [types, setTypes] = useState<CreatedTypeDef[]>([]);
  const { showNotification } = useNotification();
  const { maxTree } = useMatcher();
  const matchPattern = useMatcherFn();

  const reload = useCallback(() => {
    commands
      .listCreatedTypes(dbName)
      .then((ts) => {
        setTypes(ts);
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "list types failed", message: e });
      });
  }, [showNotification, dbName]);

  const showItems = useMemo(() => {
    return types.filter((e) => matchPattern(e.name)).slice(0, maxTree);
  }, [types, maxTree, matchPattern]);

  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  useEffect(() => {
    reload();
  }, [reload]);

  const idForTrees = useRef(uuidv4());
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
        <ShowingCount showing={showItems.length} total={types.length} />
      </Stack>>
      {showItems.map((typ) => {
        const id = `${dbName}-type-${typ.name}`;
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
                      dbName={dbName}
                      kind={kind}
                      createdType={typ}
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
    </TreeItem>
  );
}

function TypeMenu(props: {
  dbName: string;
  createdType: CreatedTypeDef;
  reload: () => void;
  kind: DataSourceKind;
}) {
  const { dbName, createdType, reload, kind } = props;
  const { appendTab } = useTabsManage();
  const { showNotification } = useNotification();
  const handleOpenDetails = () => {
    appendTab(
      new TabContent(
        createdType.name,
        <DefAndMetasView
          reloadOP={async () => {
            return Promise.resolve({
              def: createdType.sql,
              metas: [],
            });
          }}
        />,
        <PiTreeStructureFill />,
        dbName,
        kind,
      ),
    );
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

    commands
      .dropCreatedType(dbName, createdType.name)
      .then((sql) => {
        showNotification({
          severity: "success",
          title: "Drop type OK",
          message: <CopySql sql={sql} />,
        });
        reload();
      })

      .catch((e) => {
        showNotification({ severity: "error", title: "Drop type Failure", message: e });
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
