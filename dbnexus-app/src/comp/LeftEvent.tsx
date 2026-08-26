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
import { MdOutlineEventRepeat } from "react-icons/md";
import {
  DataSourceKind,
  secondConfirm,
  TabContent,
  textEllipsisStyle,
  DbnameContext,
} from "./helper";
import { MenuRefresh, VirtualSubSome } from "./CommonComp";
import { CopySql, ShowingCount } from "./CommonComp2";
import { useContextMenu, useNotification, useTabsManage, useMatcherFn } from "./store";
import { commands, SqlEvent } from "./bindings";
import { uuidv4 } from "./Common";
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
                <MenuRefresh refresh={reload} />
              </MenuList>
            ),
            event: e,
          });
        }}>
        <MdOutlineEventRepeat />
        <span> events </span>
        <ShowingCount showing={showItems.length} total={events.length} />
      </Stack>>
      <VirtualSubSome
        data={showItems}
        makeMenu={(event) => {
          return (
            <EventMenu reload={reload} dbName={dbName} kind={kind} event={event} />
          );
        }}
        show={(event) => {
          return (
            <>
              <MdOutlineEventRepeat />
              <span style={textEllipsisStyle}> {event.name} </span>
            </>
          );
        }}
      />
    </TreeItem>
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
