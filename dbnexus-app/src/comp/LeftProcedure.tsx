import { ListItemIcon, ListItemText, MenuItem, MenuList, Stack } from "@mui/material";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import { useCallback, useContext, useEffect, useRef, useState } from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import InfoIcon from "@mui/icons-material/Info";
import { tabsHasId } from "./Common";
import types from "./ApiStruct";
import { invoke } from "@tauri-apps/api/core";
import { TabContent } from "./Tabs";
import { DefAndMetasView } from "./DefAndMetaView";

import { SiProcessingfoundation } from "react-icons/si";
import {
  ConnTypeContext,
  DataSourceKind,
  generateRandomString,
  secondConfirm,
  UuidContext,
} from "./helper";
import { ExceedMaxTreeItem, MenuRefresh, UdfNameAndParameters } from "./CommonComp";
import { listProcedure, makeShowProcedureExtraArg, showProcedure } from "./handlers";
import { CopySql } from "./CommonComp2";
import {
  useAppConfigStore,
  useContextMenu,
  useNotification,
  useTabsManage,
} from "./store";
function ProceduresTree() {
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const [ps, setPos] = useState<types.NamedStaff[]>([]);
  const { showNotification } = useNotification((s) => s);
  const { matchPattern, maxTree } = useAppConfigStore((state) => state);
  const reload = useCallback(() => {
    listProcedure(uuid)
      .then((ts) => {
        setPos(ts);
      })
      .catch((e) => {
        showNotification("error", "list procedure failed", e);
      });
  }, [showNotification, uuid]);

  const [exceed, setExceed] = useState(0);
  useEffect(() => {
    setExceed(ps.filter((e) => matchPattern(e.name)).length);
  }, [ps, matchPattern]);

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
        }}
      >
        <SiProcessingfoundation />
        <span> procedures </span>
      </Stack>
    >
      {ps
        .filter((e) => matchPattern(e.name))
        .map((p, index) => {
          if (index >= maxTree) {
            return;
          }
          const id = `${uuid}-${p.name}-${p.rand}`;
          return (
            <TreeItem
              key={p.name + "-" + p.parameters}
              itemId={id}
              style={{ alignItems: "center" }}
              label=<Stack
                direction="row"
                onContextMenu={(e) => {
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
      <ExceedMaxTreeItem maxTreeSize={maxTree} exceed={exceed} />
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
  const { setCurrentTab, tabs, appendTab } = useTabsManage((state) => state);
  const { showNotification } = useNotification((s) => s);

  const handleOpenDetails = () => {
    const index = tabsHasId(tabs, id);
    if (typeof index == "number") {
      setCurrentTab(index);
    } else {
      appendTab(
        new TabContent(
          id,
          procedure.name,
          <DefAndMetasView
            reloadOP={async () => {
              return showProcedure(uuid, procedure, kind);
            }}
            kind={kind}
            uuid={uuid}
          />,
          <SiProcessingfoundation className="tab-icon-size" />,
        ),
      );
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
      .then((sql) => {
        showNotification("success", "Drop procedure OK", <CopySql sql={sql} />);
        reload();
      })
      .catch((e) => {
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
