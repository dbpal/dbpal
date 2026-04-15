import { ListItemIcon, ListItemText, MenuItem, MenuList, Stack } from "@mui/material";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import { useCallback, useContext, useEffect, useRef, useState } from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import InfoIcon from "@mui/icons-material/Info";
import { DefAndMetasView } from "./DefAndMetaView";
import { MdOutlineEventRepeat } from "react-icons/md";
import {
  DataSourceKind,
  secondConfirm,
  TabContent,
  textEllipsisStyle,
  UuidContext,
} from "./helper";
import { v4 as uuidv4 } from "uuid";
import { ExceedMaxTreeItem, MenuRefresh } from "./CommonComp";
import { CopySql, ShowingCount } from "./CommonComp2";
import {
  useMatcher,
  useContextMenu,
  useNotification,
  useTabsManage,
  useMatcherFn,
} from "./store";
import { commands, DefAndMetas, SqlEvent } from "./bindings";
function EventsTree() {
  const { uuid, kind } = useContext(UuidContext);
  const [evens, setEvents] = useState<SqlEvent[]>([]);
  const { showNotification } = useNotification();
  const { maxTree } = useMatcher();
  const matchPattern = useMatcherFn();
  const reload = useCallback(() => {
    commands
      .listEvent(uuid)
      .then((ts) => {
        setEvents(ts);
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "list event failed", message: e });
      });
  }, [showNotification, uuid]);
  const [showItems, setShowItems] = useState(0);

  useEffect(() => {
    setShowItems(evens.filter((e) => matchPattern(e.name)).length);
  }, [evens, matchPattern]);

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
        <MdOutlineEventRepeat />
        <span> events </span>
        <ShowingCount showing={showItems} total={evens.length} />
      </Stack>>
      {evens
        .filter((event) => matchPattern(event.name))
        .map((event, index) => {
          if (index >= maxTree) {
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
                      />
                    ),
                    event: e,
                  });
                }}
                sx={{ alignItems: "center" }}>
                <MdOutlineEventRepeat />
                <span style={textEllipsisStyle}> {event.name} </span>
              </Stack>
            />
          );
        })}
      <ExceedMaxTreeItem showItems={showItems} maxTreeSize={maxTree} />
    </TreeItem>
  );
}

function EventMenu(props: { uuid; event: SqlEvent; reload; kind: DataSourceKind }) {
  const { uuid, event, reload, kind } = props;
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
            } as DefAndMetas);
          }}
          kind={kind}
          uuid={uuid}
        />,
        <MdOutlineEventRepeat />,
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
      .dropEvent(uuid, event.name)
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
