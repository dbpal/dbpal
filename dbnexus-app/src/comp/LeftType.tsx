import { ListItemIcon, ListItemText, MenuItem, MenuList } from "@mui/material";
import { useCallback, useContext, useEffect, useMemo, useState } from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import InfoIcon from "@mui/icons-material/Info";
import { DefAndMetasView } from "./DefAndMetaView";
import { PiTreeStructureFill } from "react-icons/pi";
import {
  DataSourceKind,
  secondConfirm,
  TabContent,
  textEllipsisStyle,
  DbnameContext,
} from "./helper";
import { MenuRefresh, MyTreeItem, VirtualListLeft } from "./CommonComp";
import { CopySql, ShowingCount } from "./CommonComp2";
import { useNotification, useTabsManage, useMatcherFn } from "./store";
import { commands, CreatedTypeDef } from "./bindings";
function TypesTree() {
  const { dbName, kind } = useContext(DbnameContext);
  const [types, setTypes] = useState<CreatedTypeDef[]>([]);
  const { showNotification } = useNotification();
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
    return types.filter((e) => matchPattern(e.name));
  }, [types, matchPattern]);

  useEffect(() => {
    reload();
  }, [reload]);

  return (
    <MyTreeItem
      enable={true}
      makeMenu={() => {
        return (
          <MenuList>
            <MenuRefresh refresh={reload} />
          </MenuList>
        );
      }}
      label=<>
        <PiTreeStructureFill />
        <span> types </span>
        <ShowingCount showing={showItems.length} total={types.length} />
      </>>
      <VirtualListLeft
        data={showItems}
        makeMenu={(typ) => {
          return (
            <TypeMenu reload={reload} dbName={dbName} kind={kind} createdType={typ} />
          );
        }}
        show={(typ) => {
          return (
            <>
              <span style={textEllipsisStyle}> {typ.name} </span>
            </>
          );
        }}
      />
    </MyTreeItem>
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
