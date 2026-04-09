import {
  Divider,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Stack,
} from "@mui/material";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import { useCallback, useContext, useEffect, useRef, useState } from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import InfoIcon from "@mui/icons-material/Info";
import { MdFunctions } from "react-icons/md";
import types from "./ApiStruct";
import { invoke } from "@tauri-apps/api/core";
import { DefAndMetasView } from "./DefAndMetaView";

import { TbMathFunction } from "react-icons/tb";
import {
  DataSourceKind,
  generateRandomString,
  secondConfirm,
  TabContent,
  UuidContext,
} from "./helper";

import { ExceedMaxTreeItem, MenuRefresh, UdfNameAndParameters } from "./CommonComp";
import { listCassandraAgg, listUdf, makeUdfExtraArg, showUdf } from "./handlers";
import { showCassandraAgg } from "./handlers";
import { CopySql } from "./CommonComp2";
import {
  useMatcher,
  useContextMenu,
  useNotification,
  useTabsManage,
  useMatcherFn,
} from "./store";

function UdfsTree() {
  const { uuid, kind } = useContext(UuidContext);

  const [udfs, setUdfs] = useState<types.NamedStaff[]>([]);
  const { showNotification } = useNotification();
  const [exceed, setExceed] = useState(0);
  const { maxTree } = useMatcher();
  const matchPattern = useMatcherFn();
  useEffect(() => {
    setExceed(udfs.filter((e) => matchPattern(e.name)).length);
  }, [udfs, matchPattern]);

  const reload = useCallback(() => {
    listUdf(uuid)
      .then((e) => {
        setUdfs(e);
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "list udf failed", message: e });
      });
  }, [showNotification, uuid]);
  useEffect(() => {
    reload();
  }, [reload]);

  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const treeID = useRef(generateRandomString());
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
      </Stack>>
      {udfs
        .filter((e) => matchPattern(e.name))
        .map((udf, index) => {
          if (index >= maxTree) {
            return;
          }
          const id = `${uuid}-udf-${udf.name} -${udf.rand}`;
          return (
            <TreeItem
              key={index}
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
      <ExceedMaxTreeItem exceed={exceed} maxTreeSize={maxTree} />
    </TreeItem>
  );
}

function UdfMenu(props: { uuid; udf: types.NamedStaff; reload; kind: DataSourceKind }) {
  const { uuid, udf, reload, kind } = props;
  const { appendTab } = useTabsManage();
  const { showNotification } = useNotification();
  const handleOpenDetails = () => {
    appendTab(
      new TabContent(
        udf.name,
        <DefAndMetasView
          reloadOP={async () => {
            return showUdf(uuid, udf, kind);
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
    invoke("drop_udf", { name: uuid, udf: udf.name, arg: makeUdfExtraArg(udf, kind) })
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
            return showCassandraAgg(uuid, agg, parameters);
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

    invoke("cassandra_drop_aggregate", {
      name: uuid,
      aggName: agg,
      parameters,
    })
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
  const [aggs, setAggs] = useState<types.NamedStaff[]>([]);
  const { showNotification } = useNotification();
  const reload = useCallback(() => {
    listCassandraAgg(uuid)
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
  const [exceed, setExceed] = useState(0);
  const { maxTree } = useMatcher();
  const matchPattern = useMatcherFn();
  useEffect(() => {
    setExceed(aggs.filter((e) => matchPattern(e.name)).length);
  }, [aggs, matchPattern]);
  useEffect(() => {
    reload();
  }, [reload]);
  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const tressId = useRef(generateRandomString());
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
      </Stack>>
      {aggs
        .filter((e) => matchPattern(e.name))
        .map((agg, index) => {
          if (index >= maxTree) {
            return;
          }
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
      <ExceedMaxTreeItem exceed={exceed} maxTreeSize={maxTree} />
    </TreeItem>
  );
}

export { UdfsTree, CassandraAggregatesTree };
