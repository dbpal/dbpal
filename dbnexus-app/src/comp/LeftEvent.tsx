import { ListItemIcon, ListItemText, MenuItem, MenuList } from "@mui/material";
import { useCallback, useContext, useEffect, useMemo, useState } from "react";
import { DeleteForeverIcon, InfoIcon } from "./icons";
import { DefAndMetasView } from "./DefAndMetaView";
import { MdOutlineEventRepeat } from "./icons";
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
import { commands, SqlEvent } from "./bindings";
function EventsTree() {
  const { dbName, kind } = useContext(DbnameContext);
  const [events, setEvents] = useState<SqlEvent[]>([]);
  const { showNotification } = useNotification();
  const matchPattern = useMatcherFn();
  const reload = useCallback(() => {
    commands
      .listEvent(dbName)
      .then((ts) => {
        setEvents(ts);
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "list event failed", message: e });
      });
  }, [showNotification, dbName]);
  const showItems = useMemo(() => {
    return events.filter((e: SqlEvent) => matchPattern(e.name));
  }, [events, matchPattern]);

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
        <MdOutlineEventRepeat />
        <span> events </span>
        <ShowingCount showing={showItems.length} total={events.length} />
      </>>
      <VirtualListLeft
        data={showItems}
        makeMenu={(event) => {
          return (
            <EventMenu reload={reload} dbName={dbName} kind={kind} event={event} />
          );
        }}
        show={(event) => {
          return (
            <>
              <span style={textEllipsisStyle}> {event.name} </span>
            </>
          );
        }}
      />
    </MyTreeItem>
  );
}

type EventMenuProps = {
  dbName: string;
  event: SqlEvent;
  reload: () => void;
  kind: DataSourceKind;
};

function EventMenu(props: EventMenuProps) {
  const { dbName, event, reload, kind } = props;
  const { appendTab } = useTabsManage();
  const { showNotification } = useNotification();
  const handleOpenDetails = () => {
    appendTab(
      new TabContent(
        event.name,
        <DefAndMetasView
          reloadOP={async () => {
            return Promise.resolve({
              def: event.sql,
              metas: event.metas,
            });
          }}
        />,

        <MdOutlineEventRepeat />,
        dbName,
        kind,
      ),
    );
  };
  const handleDrop = async () => {
    const confirm = await secondConfirm({
      title: `Drop event '${event.name}'`,
      confirmText: "Drop",
      kind: "error",
    });
    if (!confirm) {
      return;
    }
    commands
      .dropEvent(dbName, event.name)
      .then((sql) => {
        showNotification({
          severity: "success",
          title: "Drop event OK",
          message: <CopySql sql={sql} />,
        });
        reload();
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "Drop event failed", message: e });
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

export { EventsTree };
