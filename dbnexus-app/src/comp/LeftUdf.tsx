import { Divider, ListItemIcon, ListItemText, MenuItem, MenuList } from "@mui/material";
import { useCallback, useContext, useEffect, useMemo, useState } from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import InfoIcon from "@mui/icons-material/Info";
import { MdFunctions } from "react-icons/md";
import { DefAndMetasView } from "./DefAndMetaView";
import { TbMathFunction } from "react-icons/tb";
import { DataSourceKind, secondConfirm, TabContent, DbnameContext } from "./helper.ts";
import {
  MenuRefresh,
  MyTreeItem,
  UdfNameAndParameters,
  VirtualListLeft,
} from "./CommonComp";
import { CopySql, ShowingCount } from "./CommonComp2";
import { useNotification, useTabsManage, useMatcherFn } from "./store";
import { commands, NamedStaff } from "./bindings.ts";

function UdfsTree() {
  const { dbName, kind } = useContext(DbnameContext);

  const [udfs, setUdfs] = useState<NamedStaff[]>([]);
  const { showNotification } = useNotification();
  const matchPattern = useMatcherFn();
  const showItems = useMemo(() => {
    return udfs.filter((e) => matchPattern(e.name));
  }, [udfs, matchPattern]);

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
      label=<>
        <TbMathFunction />
        <span> udfs </span>
        <ShowingCount showing={showItems.length} total={udfs.length} />
      </>>
      <VirtualListLeft
        data={showItems}
        makeMenu={(e) => {
          return <UdfMenu reload={reload} dbName={dbName} kind={kind} udf={e} />;
        }}
        show={(udf) => {
          return (
            <>
              <UdfNameAndParameters udf={udf} kind={kind} />
            </>
          );
        }}
      />
    </MyTreeItem>
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
  const matchPattern = useMatcherFn();
  const showItems = useMemo(() => {
    return aggs.filter((e) => matchPattern(e.name));
  }, [aggs, matchPattern]);

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
      label=<>
        <MdFunctions />
        <span> aggregates </span>
        <ShowingCount showing={showItems.length} total={aggs.length} />
      </>>
      <VirtualListLeft
        data={aggs}
        makeMenu={(agg) => {
          return (
            <CassandraAggregateMenu
              reload={reload}
              dbName={dbName}
              agg={agg.name}
              parameters={agg.cassandra_parameters}
            />
          );
        }}
        show={(agg) => {
          return (
            <>
              <UdfNameAndParameters udf={agg} kind={DataSourceKind.Cassandra} />
            </>
          );
        }}
      />
    </MyTreeItem>
  );
}

export { UdfsTree, CassandraAggregatesTree };
