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
import { useCallback, useContext, useEffect, useMemo, useState } from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import InfoIcon from "@mui/icons-material/Info";
import { DefAndMetasView } from "./DefAndMetaView.tsx";
import {
  DataSourceKind,
  secondConfirm,
  TabContent,
  textEllipsisStyle,
  DbnameContext,
} from "./helper.ts";

import { MenuRefresh, MyTreeItem, VirtualListLeft } from "./CommonComp.tsx";
import { GrSteps } from "react-icons/gr";
import { CopySql, ShowingCount } from "./CommonComp2.tsx";
import {
  useNotification,
  useTabsManage,
  useMatcherFn,
  useOneShotDialogUI,
} from "./store.tsx";
import { commands, DefAndMetas, SequenceDetail } from "./bindings.ts";
function SequenceTree() {
  const { dbName, kind } = useContext(DbnameContext);
  const [seq, setSeq] = useState<SequenceDetail[]>([]);
  const { showNotification } = useNotification();
  const matchPattern = useMatcherFn();
  const reload = useCallback(() => {
    commands
      .listSequence(dbName)
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
  }, [showNotification, dbName]);

  const showItems = useMemo(() => {
    return seq.filter((e) => matchPattern(e.name));
  }, [seq, matchPattern]);
  useEffect(() => {
    reload();
  }, [reload]);
  return (
    <MyTreeItem
      conn_established={true}
      makeMenu={() => {
        return (
          <MenuList>
            <MenuRefresh refresh={reload} />
          </MenuList>
        );
      }}
      label={
        <>
          <GrSteps />
          <span> sequences </span>
          <ShowingCount showing={showItems.length} total={seq.length} />
        </>
      }>
      <VirtualListLeft
        data={showItems}
        makeMenu={(seq) => {
          return <SequenceMenu reload={reload} dbName={dbName} kind={kind} seq={seq} />;
        }}
        show={(seq) => {
          return (
            <>
              <GrSteps />
              <span style={textEllipsisStyle}>{seq.name}</span>
            </>
          );
        }}
      />
    </MyTreeItem>
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
  dbName: string;
  seq: SequenceDetail;
  reload: () => void;
  kind: DataSourceKind;
}) {
  const { dbName, seq, reload, kind } = props;
  const { appendTab } = useTabsManage();
  const { setOneShotUI } = useOneShotDialogUI();

  const handleOpenDetails = () => {
    appendTab(
      new TabContent(
        seq.name,
        <DefAndMetasView
          reloadOP={() => {
            return Promise.resolve(details2DefMetas(seq));
          }}
        />,
        <GrSteps />,
        dbName,
        kind,
      ),
    );
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
          setOneShotUI({
            ui: (
              <DropSeq
                kind={kind}
                seq={seq}
                handleClose={() => {
                  setOneShotUI({ ui: null });
                }}
                reload={reload}
                dbName={dbName}
              />
            ),
            title: `drop sequence ${seq.name}`,
          });
        }}>
        <ListItemIcon>
          <DeleteForeverIcon color="error" />
        </ListItemIcon>
        <ListItemText> Drop</ListItemText>
      </MenuItem>
    </MenuList>
  );
}

function DropSeq(props: {
  kind: DataSourceKind;
  seq: SequenceDetail;
  reload: () => void;
  handleClose: () => void;
  dbName;
}) {
  const { kind, seq, reload, handleClose, dbName } = props;

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
      .dropSequence(dbName, seq.name, cascade)
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
