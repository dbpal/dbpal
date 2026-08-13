import { ListItemIcon, ListItemText, MenuItem, MenuList, Stack } from "@mui/material";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import {
  MouseEvent,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import InfoIcon from "@mui/icons-material/Info";
import { DefAndMetasView } from "./DefAndMetaView";

import { SiProcessingfoundation } from "react-icons/si";
import { DataSourceKind, secondConfirm, TabContent, DbnameContext } from "./helper.ts";
import { MenuRefresh, UdfNameAndParameters } from "./CommonComp";

import { CopySql, ShowingCount } from "./CommonComp2";
import {
  useMatcher,
  useContextMenu,
  useNotification,
  useTabsManage,
  useMatcherFn,
} from "./store";
import { commands, NamedStaff } from "./bindings.ts";
import { uuidv4 } from "./Common.ts";
function ProceduresTree() {
  const { dbName, kind } = useContext(DbnameContext);
  const [ps, setPs] = useState<NamedStaff[]>([]);
  const { showNotification } = useNotification();
  const { maxTree } = useMatcher();
  const matchPattern = useMatcherFn();
  const reload = useCallback((): void => {
    commands
      .listProcedure(dbName)
      .then((data) => {
        setPs(data);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "list procedure failed",
          message: e,
        });
      });
  }, [showNotification, dbName]);

  const showItems = useMemo(() => {
    return ps.filter((e) => matchPattern(e.name)).slice(0, maxTree);
  }, [ps, maxTree, matchPattern]);

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
        onContextMenu={(e: MouseEvent<HTMLDivElement>) => {
          setUpMenu({
            content: (
              <MenuList>
                <MenuRefresh reload={reload} />
              </MenuList>
            ),
            event: e,
          });
        }}>
        <SiProcessingfoundation />
        <span> procedures </span>
        <ShowingCount showing={showItems.length} total={ps.length} />
      </Stack>>
      {showItems.map((p) => {
        const id = `${dbName}-${p.name}-${p.rand}`;
        return (
          <TreeItem
            key={p.name + "-" + p.parameters}
            itemId={id}
            style={{ alignItems: "center" }}
            label=<Stack
              direction="row"
              onContextMenu={(e: MouseEvent<HTMLDivElement>) => {
                setUpMenu({
                  content: (
                    <ProcedureMenu
                      reload={reload}
                      dbName={dbName}
                      kind={kind}
                      procedure={p}
                    />
                  ),
                  event: e,
                });
              }}
              sx={{ alignItems: "center" }}>
              <SiProcessingfoundation />
              <UdfNameAndParameters udf={p} kind={kind} />
            </Stack>
          />
        );
      })}
    </TreeItem>
  );
}

function ProcedureMenu(props: {
  dbName: string;
  procedure: NamedStaff;
  reload: () => void;
  kind: DataSourceKind;
}) {
  const { dbName, procedure, reload, kind } = props;
  const { appendTab } = useTabsManage();
  const { showNotification } = useNotification();

  const handleOpenDetails = (): void => {
    appendTab(
      new TabContent(
        procedure.name,
        <DefAndMetasView
          reloadOP={async () => {
            return commands
              .showProcedure(dbName, procedure)
              .then((data) => {
                return {
                  def: data.def,
                  metas: data.metas,
                };
              })
              .catch((e) => {
                showNotification({
                  severity: "error",
                  title: "Show procedure failed",
                  message: e,
                });
                return { def: "", metas: [] };
              });
          }}
        />,
        <SiProcessingfoundation />,
        dbName,
        kind,
      ),
    );
  };

  const handleDrop = async (): Promise<void> => {
    const confirm = await secondConfirm({
      title: `Drop Procedure '${procedure}'`,
      confirmText: "Drop",
      kind: "error",
    });
    if (!confirm) {
      return;
    }

    commands
      .dropProcedure(dbName, procedure)
      .then((sql) => {
        showNotification({
          severity: "success",
          title: "Drop procedure OK",
          message: <CopySql sql={sql} />,
        });
        reload();
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "Drop procedure Failure",
          message: e,
        });
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

export { ProceduresTree };
