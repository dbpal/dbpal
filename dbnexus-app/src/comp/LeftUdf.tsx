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
import { DataSourceKind, secondConfirm, TabContent, UuidContext } from "./helper.ts";
import { v4 as uuidv4 } from "uuid";
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

function UdfsTree() {
  const { uuid, kind } = useContext(UuidContext);

  const [udfs, setUdfs] = useState<NamedStaff[]>([]);
  const { showNotification } = useNotification();
  const { maxTree } = useMatcher();
  const matchPattern = useMatcherFn();
  const showItems = useMemo(() => {
    return udfs.filter((e) => matchPattern(e.name)).slice(0, maxTree);
  }, [udfs, maxTree, matchPattern]);

  const reload = useCallback(() => {
    commands
      .listUdf(uuid)
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
  }, [showNotification, uuid]);
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
                <MenuRefresh reload={reload} />
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
        const id = `${uuid}-udf-${udf.name} -${udf.rand}`;
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
                    <UdfMenu reload={reload} uuid={uuid} kind={kind} udf={udf} />
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

function UdfMenu(props: { uuid; udf: NamedStaff; reload; kind: DataSourceKind }) {
  const { uuid, udf, reload, kind } = props;
  const { appendTab } = useTabsManage();
  const { showNotification } = useNotification();
  const handleOpenDetails = () => {
    appendTab(
      new TabContent(
        udf.name,
        <DefAndMetasView
          reloadOP={async () => {
            return commands.showUdf(uuid, udf);
          }}
          kind={kind}
          uuid={uuid}
        />,
        <TbMathFunction />,
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
      .dropUdf(uuid, udf)
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
  uuid: string;
  agg: string;
  parameters: string[] | null;
  reload;
}) {
  const { uuid, agg, reload, parameters } = props;
  const { appendTab } = useTabsManage();
  const { showNotification } = useNotification();

  const handleOpenDetails = () => {
    appendTab(
      new TabContent(
        agg,
        <DefAndMetasView
          reloadOP={async () => {
            return await commands.cassandraShowAggregate(uuid, agg, parameters);
          }}
          uuid={uuid}
          kind={DataSourceKind.Cassandra}
        />,
        <MdFunctions />,
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
      .cassandraDropAggregate(uuid, agg, parameters)
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
    </div>
  );
}

function CassandraAggregatesTree() {
  const { uuid } = useContext(UuidContext);
  const [aggs, setAggs] = useState<NamedStaff[]>([]);
  const { showNotification } = useNotification();
  const reload = useCallback(() => {
    commands
      .cassandraListAggregate(uuid)
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
  }, [showNotification, uuid]);
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
                <MenuRefresh reload={reload} />
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
        const id = `${uuid}-agg-${agg.name}-${agg.rand}`;
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
                      uuid={uuid}
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
