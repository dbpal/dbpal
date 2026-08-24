import {
  Divider,
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
import { MdFunctions } from "react-icons/md";
import { DefAndMetasView } from "./DefAndMetaView";

import { TbMathFunction } from "react-icons/tb";
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

function UdfsTree() {
  const { dbName, kind } = useContext(DbnameContext);

  const [udfs, setUdfs] = useState<NamedStaff[]>([]);
  const { showNotification } = useNotification();
  const { maxTree } = useMatcher();
  const matchPattern = useMatcherFn();
  const showItems = useMemo(() => {
    return udfs.filter((e) => matchPattern(e.name)).slice(0, maxTree);
  }, [udfs, maxTree, matchPattern]);

  const reload = useCallback(() => {
    commands
      .listUdf(dbName)
      .then((data) => {
        setUdfs(data);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "list udf failed",
          message: e,
        });
      });
  }, [showNotification, dbName]);
  useEffect(() => {
    reload();
  }, [reload]);

  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const treeID = useRef(uuidv4());
  return (
    <TreeItem
      itemId={treeID.current}
      label=<Stack
        direction="row"
        sx={{ alignItems: "center" }}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <MenuList>
                <MenuRefresh refresh={reload} />
              </MenuList>
            ),
            event: e,
          });
        }}>
        <TbMathFunction />
        <span> udfs </span>
        <ShowingCount showing={showItems.length} total={udfs.length} />
      </Stack>>
      {showItems.map((udf) => {
        const id = `${dbName}-udf-${udf.name} -${udf.rand}`;
        return (
          <TreeItem
            key={id}
            itemId={id}
            label=<Stack
              direction="row"
              sx={{ alignItems: "center" }}
              onContextMenu={(e) => {
                setUpMenu({
                  content: (
                    <UdfMenu reload={reload} dbName={dbName} kind={kind} udf={udf} />
                  ),
                  event: e,
                });
              }}>
              <TbMathFunction />
              <UdfNameAndParameters udf={udf} kind={kind} />
            </Stack>
          />
        );
      })}
    </TreeItem>
  );
}

function UdfMenu(props: { dbName; udf: NamedStaff; reload; kind: DataSourceKind }) {
  const { dbName, udf, reload, kind } = props;
  const { appendTab } = useTabsManage();
  const { showNotification } = useNotification();
  const handleOpenDetails = () => {
    appendTab(
      new TabContent(
        udf.name,
        <DefAndMetasView
          reloadOP={async () => {
            return commands.showUdf(dbName, udf);
          }}
        />,
        <TbMathFunction />,
        dbName,
        kind,
      ),
    );
  };

  const handleDrop = async () => {
    const confirm = await secondConfirm({
      title: `Drop UDF '${udf.name}'`,
      confirmText: "Drop",
      kind: "error",
    });
    if (!confirm) {
      return;
    }
    commands
      .dropUdf(dbName, udf)
      .then((sql) => {
        showNotification({
          severity: "success",
          title: "Drop udf OK",
          message: <CopySql sql={sql} />,
        });
        reload();
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "Drop udf failure", message: e });
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
      <Divider />
      <MenuItem
        sx={{ color: "error.main" }}
        onClick={() => {
          handleDrop();
        }}>
        <ListItemIcon>
          <DeleteForeverIcon color="error" />
        </ListItemIcon>
        <ListItemText>Drop</ListItemText>
      </MenuItem>
    </MenuList>
  );
}

function CassandraAggregateMenu(props: {
  dbName: string;
  agg: string;
  parameters: string[] | null;
  reload;
}) {
  const { dbName, agg, reload, parameters } = props;
  const { appendTab } = useTabsManage();
  const { showNotification } = useNotification();

  const handleOpenDetails = () => {
    appendTab(
      new TabContent(
        agg,
        <DefAndMetasView
          reloadOP={async () => {
            return await commands.cassandraShowAggregate(dbName, agg, parameters);
          }}
        />,
        <MdFunctions />,
        dbName,
        DataSourceKind.Cassandra,
      ),
    );
  };

  const handleDrop = async () => {
    const confirm = await secondConfirm({
      title: `Drop Aggregate '${agg}'`,
      confirmText: "Drop",
      kind: "error",
    });
    if (!confirm) {
      return;
    }

    commands
      .cassandraDropAggregate(dbName, agg, parameters)
      .then((sql) => {
        showNotification({
          severity: "success",
          title: "Drop aggregate OK",
          message: <CopySql sql={sql} />,
        });
        reload();
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "Drop aggregate Failure",
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

      <Divider />
      <MenuItem
        sx={{ color: "error.main" }}
        onClick={() => {
          handleDrop();
        }}>
        <ListItemIcon>
          <DeleteForeverIcon color="error" />
        </ListItemIcon>
        <ListItemText>Drop</ListItemText>
      </MenuItem>
    </MenuList>
  );
}

function CassandraAggregatesTree() {
  const { dbName } = useContext(DbnameContext);
  const [aggs, setAggs] = useState<NamedStaff[]>([]);
  const { showNotification } = useNotification();
  const reload = useCallback(() => {
    commands
      .cassandraListAggregate(dbName)
      .then((ts) => {
        setAggs(ts);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "list aggregate failed",
          message: e,
        });
      });
  }, [showNotification, dbName]);
  const { maxTree } = useMatcher();
  const matchPattern = useMatcherFn();
  const showItems = useMemo(() => {
    return aggs.filter((e) => matchPattern(e.name)).slice(0, maxTree);
  }, [aggs, matchPattern, maxTree]);

  useEffect(() => {
    reload();
  }, [reload]);
  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const tressId = useRef(uuidv4());
  return (
    <TreeItem
      itemId={tressId.current}
      label=<Stack
        direction="row"
        sx={{ alignItems: "center" }}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <MenuList>
                <MenuRefresh refresh={reload} />
              </MenuList>
            ),
            event: e,
          });
        }}>
        <MdFunctions />
        <span> aggregates </span>
        <ShowingCount showing={showItems.length} total={aggs.length} />
      </Stack>>
      {showItems.map((agg) => {
        const id = `${dbName}-agg-${agg.name}-${agg.rand}`;
        return (
          <TreeItem
            key={
              agg.name +
              "-" +
              (agg.cassandra_parameters ? agg.cassandra_parameters.join("-") : "")
            }
            itemId={id}
            label=<Stack
              direction="row"
              sx={{ alignItems: "center" }}
              onContextMenu={(e) => {
                setUpMenu({
                  content: (
                    <CassandraAggregateMenu
                      reload={reload}
                      dbName={dbName}
                      agg={agg.name}
                      parameters={agg.cassandra_parameters}
                    />
                  ),
                  event: e,
                });
              }}>
              <MdFunctions />
              <UdfNameAndParameters udf={agg} kind={DataSourceKind.Cassandra} />
            </Stack>
          />
        );
      })}
    </TreeItem>
  );
}

export { UdfsTree, CassandraAggregatesTree };
