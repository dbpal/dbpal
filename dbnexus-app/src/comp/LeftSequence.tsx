import {
  Button,
  Checkbox,
  FormControlLabel,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Stack,
} from "@mui/material";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import { useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import InfoIcon from "@mui/icons-material/Info";
import { DefAndMetasView } from "./DefAndMetaView.tsx";
import {
  DataSourceKind,
  secondConfirm,
  TabContent,
  textEllipsisStyle,
  UuidContext,
} from "./helper.ts";
import { v4 as uuidv4 } from "uuid";

import { MenuRefresh } from "./CommonComp.tsx";
import { GrSteps } from "react-icons/gr";
import { CopySql, ShowingCount } from "./CommonComp2.tsx";
import {
  useMatcher,
  useContextMenu,
  useNotification,
  useTabsManage,
  useMatcherFn,
  useOneShotDialogUI,
} from "./store.tsx";
import { commands, DefAndMetas, SequenceDetail } from "./bindings.ts";
function SequenceTree() {
  const { uuid, kind } = useContext(UuidContext);
  const [seq, setSeq] = useState<SequenceDetail[]>([]);
  const { showNotification } = useNotification();
  const { maxTree } = useMatcher();
  const matchPattern = useMatcherFn();
  const reload = useCallback(() => {
    commands
      .listSequence(uuid)
      .then((ts) => {
        setSeq(ts);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "list sequence failed",
          message: e,
        });
      });
  }, [showNotification, uuid]);

  const showItems = useMemo(() => {
    return seq.filter((e) => matchPattern(e.name)).slice(0, maxTree);
  }, [seq, maxTree, matchPattern]);
  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  useEffect(() => {
    reload();
  }, [reload]);
  const idForTree = useRef(uuidv4());
  return (
    <TreeItem
      itemId={idForTree.current}
      label={
        <Stack
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
          <GrSteps />
          <span> sequences </span>
          <ShowingCount showing={showItems.length} total={seq.length} />
        </Stack>
      }>
      {showItems.map((seq, index) => {
        if (index >= maxTree) {
          return;
        }
        const id = `${uuid}-sequence-${seq.name}`;
        return (
          <TreeItem
            key={seq.name}
            itemId={id}
            style={{ alignItems: "center" }}
            label={
              <Stack
                direction="row"
                onContextMenu={(e) => {
                  setUpMenu({
                    content: (
                      <SequenceMenu reload={reload} uuid={uuid} kind={kind} seq={seq} />
                    ),
                    event: e,
                  });
                }}
                sx={{ alignItems: "center" }}>
                <GrSteps />
                <span style={textEllipsisStyle}>{seq.name}</span>
              </Stack>
            }
          />
        );
      })}
    </TreeItem>
  );
}

function details2DefMetas(d: SequenceDetail): DefAndMetas {
  return {
    def: d.sql,
    metas: [
      { name: "data type", value: d.data_type },
      { name: "start_value", value: d.start_value },
      { name: "min value", value: d.min_value },
      { name: "max value", value: d.max_value },
      { name: "increment by", value: d.increment_by },
      { name: "last value", value: d.last_value },
    ],
  };
}

function SequenceMenu(props: {
  uuid: string;
  seq: SequenceDetail;
  reload: () => void;
  kind: DataSourceKind;
}) {
  const { uuid, seq, reload, kind } = props;
  const { appendTab } = useTabsManage();
  const { setUi } = useOneShotDialogUI();

  const handleOpenDetails = () => {
    appendTab(
      new TabContent(
        seq.name,
        <DefAndMetasView
          reloadOP={() => {
            return Promise.resolve(details2DefMetas(seq));
          }}
          kind={kind}
          uuid={uuid}
        />,
        <GrSteps />,
      ),
    );
  };

  return (
    <div>
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
            setUi(
              <DropSeq
                kind={kind}
                seq={seq}
                handleClose={() => {
                  setUi(null);
                }}
                reload={reload}
                uuid={uuid}
              />,
              `drop sequence ${seq.name}`,
            );
          }}>
          <ListItemIcon>
            <DeleteForeverIcon color="error" />
          </ListItemIcon>
          <ListItemText> Drop</ListItemText>
        </MenuItem>
      </MenuList>
    </div>
  );
}

function DropSeq(props: {
  kind: DataSourceKind;
  seq: SequenceDetail;
  reload: () => void;
  handleClose: () => void;
  uuid;
}) {
  const { kind, seq, reload, handleClose, uuid } = props;

  const [cascade, setCascade] = useState(false);
  const supportCascade = [DataSourceKind.Postgres, DataSourceKind.Cockroach].includes(
    kind,
  );
  const { showNotification } = useNotification();

  const handleDrop = async () => {
    const confirm = await secondConfirm({
      title: `Drop Sequence '${seq.name}'`,
      confirmText: "Drop",
      kind: "error",
    });
    if (!confirm) {
      return;
    }
    commands
      .dropSequence(uuid, seq.name, cascade)
      .then((sql) => {
        showNotification({
          severity: "success",
          title: "Drop sequence OK",
          message: <CopySql sql={sql} />,
        });
        reload();
        handleClose();
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "Drop sequence Failure",
          message: e,
        });
      });
  };

  return (
    <Stack>
      {supportCascade && (
        <Stack>
          <FormControlLabel
            control={
              <Checkbox
                checked={cascade}
                onChange={(e) => setCascade(e.target.checked)}
              />
            }
            label="CasCade"
          />
        </Stack>
      )}
      <Stack direction="row">
        <Button
          onClick={() => {
            handleDrop();
          }}
          color="error">
          Drop
        </Button>
      </Stack>
    </Stack>
  );
}

export { SequenceTree };
