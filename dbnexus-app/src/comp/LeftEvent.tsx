"use strict";
import { ListItemIcon, ListItemText, MenuItem, MenuList, Stack } from "@mui/material";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import { useContext, useEffect, useRef, useState } from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import InfoIcon from "@mui/icons-material/Info";
import { DefAndMetas, tabsHasId } from "./Common";
import types from "./ApiStruct";
import { invoke } from "@tauri-apps/api/core";
import { NotificationContext } from "./Notification";
import { TabContent } from "./Tabs";
import { DefAndMetasView } from "./UdfAndMetaView";
import { MdOutlineEventRepeat } from "react-icons/md";
import {
  ConnTypeContext,
  ContextMenuContext,
  DataSourceKind,
  generateRandomString,
  MatchPatternContext,
  secondConfirm,
  SettingContext,
  TabsSetContext,
  UuidContext,
} from "./helper";
import { ExceedMaxTreeItem, MenuRefresh } from "./CommonComp";
import { listEvent } from "./handlers";
function EventsTree() {
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const [evens, setEvents] = useState<types.SqlEvent[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const setting = useContext(SettingContext);
  const reload = () => {
    listEvent(uuid)
      .then(ts => {
        setEvents(ts);
      })
      .catch(e => {
        showNotification("error", "list event failed", e);
      });
  };
  const [exceed, setExceed] = useState(0);
  const matchPattern = useContext(MatchPatternContext);
  useEffect(() => {
    setExceed(evens.filter(e => matchPattern(e.name)).length);
  }, [evens, matchPattern]);

  const getItemId = (x: types.SqlEvent) => {
    return `${uuid}-event-${x.name}`;
  };
  const { setUpMenu } = useContext(ContextMenuContext);

  useEffect(() => {
    reload();
  }, []);

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
                <MenuRefresh reload={reload}></MenuRefresh>
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
        .filter(event => matchPattern(event.name))
        .map((event, index) => {
          if (index >= setting.appearance.maxTreeSize) {
            return;
          }
          return (
            <TreeItem
              key={event.name}
              itemId={getItemId(event)}
              style={{ alignItems: "center" }}
              label=<Stack
                direction="row"
                onContextMenu={e => {
                  setUpMenu({
                    content: (
                      <EventMenu
                        reload={reload}
                        uuid={uuid}
                        kind={kind}
                        event={event}
                      ></EventMenu>
                    ),
                    event: e,
                  });
                }}
                sx={{ alignItems: "center" }}
              >
                <MdOutlineEventRepeat />
                <span> {event.name} </span>
              </Stack>
            ></TreeItem>
          );
        })}
      <ExceedMaxTreeItem
        exceed={exceed}
        maxTreeSize={setting.appearance.maxTreeSize}
      ></ExceedMaxTreeItem>
    </TreeItem>
  );
}

function EventMenu(props: {
  uuid;
  event: types.SqlEvent;
  reload;
  kind: DataSourceKind;
}) {
  const { uuid, event, reload, kind } = props;
  const { setSelectTab, setTabs, tabs } = useContext(TabsSetContext);
  const { showNotification } = useContext(NotificationContext);
  const handleOpenDetails = () => {
    const id = uuid + "-" + "-event-" + event.name;
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
            event.name,
            (
              <DefAndMetasView
                reloadOP={async () => {
                  return Promise.resolve({
                    def: event.sql,
                    metas: event.metas,
                  } as DefAndMetas);
                }}
                kind={kind}
                uuid={uuid}
              ></DefAndMetasView>
            ),
            <MdOutlineEventRepeat />,
          ),
        ];
      });
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
      .then(() => {
        showNotification("success", "Drop event OK");
        reload();
      })
      .catch(e => {
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
          <InfoIcon></InfoIcon>
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
          <DeleteForeverIcon color="error"></DeleteForeverIcon>
        </ListItemIcon>
        <ListItemText> Drop</ListItemText>
      </MenuItem>
    </MenuList>
  );
}

export { EventsTree };
