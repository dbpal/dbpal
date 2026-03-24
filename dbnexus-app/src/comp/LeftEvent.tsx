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
import { DefAndMetasView } from "./DefAndMetaView";
import { MdOutlineEventRepeat } from "react-icons/md";
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
import { listEvent } from "./handlers";
import { CopySql } from "./CommonComp2";
function EventsTree() {
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const [evens, setEvents] = useState<types.SqlEvent[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const maxsize = useContext(MaxTreeContext);
  const reload = useCallback(() => {
    listEvent(uuid)
      .then((ts) => {
        setEvents(ts);
      })
      .catch((e) => {
        showNotification("error", "list event failed", e);
      });
  }, [showNotification, uuid]);
  const [exceed, setExceed] = useState(0);
  const matchPattern = useContext(MatchPatternContext);
  useEffect(() => {
    setExceed(evens.filter((e) => matchPattern(e.name)).length);
  }, [evens, matchPattern]);

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
        <MdOutlineEventRepeat />
        <span> events </span>
      </Stack>
    >
      {evens
        .filter((event) => matchPattern(event.name))
        .map((event, index) => {
          if (index >= maxsize) {
            return;
          }

          const id = `${uuid}-event-${event.name}`;
          return (
            <TreeItem
              key={event.name}
              itemId={id}
              style={{ alignItems: "center" }}
              label=<Stack
                direction="row"
                onContextMenu={(e) => {
                  setUpMenu({
                    content: (
                      <EventMenu
                        reload={reload}
                        uuid={uuid}
                        kind={kind}
                        event={event}
                        id={id}
                      />
                    ),
                    event: e,
                  });
                }}
                sx={{ alignItems: "center" }}
              >
                <MdOutlineEventRepeat />
                <span> {event.name} </span>
              </Stack>
            />
          );
        })}
      <ExceedMaxTreeItem exceed={exceed} maxTreeSize={maxsize} />
    </TreeItem>
  );
}

function EventMenu(props: {
  uuid;
  event: types.SqlEvent;
  reload;
  kind: DataSourceKind;
  id: string;
}) {
  const { uuid, event, reload, kind, id } = props;
  const { setCurrentTab, tabs, appendTab } = useContext(TabsSetContext);
  const { showNotification } = useContext(NotificationContext);
  const handleOpenDetails = () => {
    const index = tabsHasId(tabs, id);
    if (typeof index == "number") {
      setCurrentTab(index);
    } else {
      appendTab(
        new TabContent(
          id,
          event.name,
          <DefAndMetasView
            reloadOP={async () => {
              return Promise.resolve({
                def: event.sql,
                metas: event.metas,
              } as DefAndMetas);
            }}
            kind={kind}
            uuid={uuid}
          />,
          <MdOutlineEventRepeat className="tab-icon-size" />,
        ),
      );
    }
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
    invoke("drop_event", { name: uuid, eventName: event.name })
      .then((sql) => {
        showNotification("success", "Drop event OK", <CopySql sql={sql} />);
        reload();
      })
      .catch((e) => {
        showNotification("error", "Drop event failed", e);
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

export { EventsTree };
