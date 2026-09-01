import { ListItemIcon, ListItemText, MenuItem, MenuList } from "@mui/material";
import { useCallback, useContext, useMemo, useState } from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import InfoIcon from "@mui/icons-material/Info";
import { DefAndMetasView } from "./DefAndMetaView";

import { SiProcessingfoundation } from "react-icons/si";
import { DataSourceKind, secondConfirm, TabContent, DbnameContext } from "./helper.ts";
import {
  MenuRefresh,
  MyTreeItem,
  UdfNameAndParameters,
  VirtualListLeft,
} from "./CommonComp";

import { CopySql, ShowingCount } from "./CommonComp2";
import { useNotification, useTabsManage, useMatcherFn } from "./store";
import { commands, NamedStaff } from "./bindings.ts";
function ProceduresTree() {
  const { dbName, kind } = useContext(DbnameContext);
  const [ps, setPs] = useState<NamedStaff[]>([]);
  const { showNotification } = useNotification();
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
    return ps.filter((e) => matchPattern(e.name));
  }, [ps, matchPattern]);

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
        <SiProcessingfoundation />
        <span> procedures </span>
        <ShowingCount showing={showItems.length} total={ps.length} />
      </>>
      <VirtualListLeft
        data={showItems}
        makeMenu={(p) => {
          return (
            <ProcedureMenu reload={reload} dbName={dbName} kind={kind} procedure={p} />
          );
        }}
        show={(p) => {
          return (
            <>
              <SiProcessingfoundation />
              <UdfNameAndParameters udf={p} kind={kind} />
            </>
          );
        }}
      />
    </MyTreeItem>
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
