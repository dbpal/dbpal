import {
  Divider,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Stack,
} from '@mui/material';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import { useContext, useEffect, useState } from 'react';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import InfoIcon from '@mui/icons-material/Info';
import { MdFunctions } from 'react-icons/md';
import { initMonacoLanague, tabsHasId } from './Common';
import types from './ApiStruct';
import { invoke } from '@tauri-apps/api/core';
import { NotificationContext } from './Notification';
import { TabContent } from './Tabs';
import { DefAndMetasView } from './UdfAndMetaView';

import { TbMathFunction } from 'react-icons/tb';
import {
  calMenuStyle,
  ConnTypeContext,
  ContextMenuContext,
  DataSourceKind,
  makeExecuteSql,
  makeGetParserErrr,
  MatchPatternContext,
  maxTreeSize,
  secondConfirm,
  TabsSetContext,
  udfIdGenerate,
  UuidContext,
} from './helper';

import {
  ExceedMaxTreeItem,
  MenuRefresh,
  UdfNameAndParameters,
} from './CommonComp';

function UdfsTree() {
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);

  const idPrefix = uuid + '-udf-';
  const [udfs, setUdfs] = useState<types.NamedStaff[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [exceed, setExceed] = useState(0);
  const matchPattern = useContext(MatchPatternContext);
  useEffect(() => {
    setExceed(udfs.filter((e) => matchPattern(e.name)).length);
  }, [udfs, matchPattern]);

  const reload = () => {
    invoke<types.NamedStaff[]>('list_udf', { name: uuid })
      .then((ts) => {
        setUdfs(ts);
      })
      .catch((e) => {
        showNotification('error', 'list udf failed', e);
      });
  };
  useEffect(() => {
    reload();
  }, []);

  let { setUpMenu } = useContext(ContextMenuContext);

  return (
    <TreeItem
      itemId={idPrefix}
      label=<Stack
        spacing={1}
        direction="row"
        sx={{ alignItems: 'center' }}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <MenuList>
                <MenuRefresh reload={reload}></MenuRefresh>
              </MenuList>
            ),
            style: calMenuStyle(e),
          });
        }}
      >
        <TbMathFunction></TbMathFunction>
        <span> udfs </span>
      </Stack>
    >
      {udfs
        .filter((e) => matchPattern(e.name))
        .map((udf, index) => {
          if (index >= maxTreeSize) {
            return;
          }
          return (
            <TreeItem
              key={udfIdGenerate(kind, udf, uuid, 'udf')}
              itemId={udfIdGenerate(kind, udf, uuid, 'udf')}
              label=<Stack
                direction="row"
                spacing={0.5}
                sx={{ alignItems: 'center' }}
                onContextMenu={(e) => {
                  setUpMenu({
                    content: (
                      <UdfMenu
                        reload={reload}
                        uuid={uuid}
                        kind={kind}
                        udf={udf}
                      ></UdfMenu>
                    ),
                    style: calMenuStyle(e),
                  });
                }}
              >
                <TbMathFunction></TbMathFunction>
                <UdfNameAndParameters
                  udf={udf}
                  kind={kind}
                ></UdfNameAndParameters>
              </Stack>
            ></TreeItem>
          );
        })}
      <ExceedMaxTreeItem exceed={exceed}></ExceedMaxTreeItem>
    </TreeItem>
  );
}

function UdfMenu(props: {
  uuid;
  udf: types.NamedStaff;
  reload;
  kind: DataSourceKind;
}) {
  const { uuid, udf, reload, kind } = props;
  const { setSelectTab, setTabs, tabs } = useContext(TabsSetContext);
  let arg = null;
  switch (kind) {
    case DataSourceKind.Postgres:
    case DataSourceKind.Cockroach:
      arg = { extra: 'postgres', parameters: udf.parameters };
      break;
    case DataSourceKind.Cassandra:
      arg = { extra: 'cassandra', parameters: udf.cassandra_parameters };
      break;
  }
  const { showNotification } = useContext(NotificationContext);
  const handleOpenDetails = () => {
    initMonacoLanague(uuid, kind);
    const id = udfIdGenerate(kind, udf, uuid, 'udf');
    let index = tabsHasId(tabs, id);
    if (typeof index == 'number') {
      setSelectTab(index);
    } else {
      setTabs((e) => {
        setSelectTab(tabs.length);
        return [
          ...e,
          new TabContent(
            id,
            udf.name,
            (
              <DefAndMetasView
                showReload
                reloadOP={async () => {
                  return await invoke<types.DefAndMetas>('show_udf', {
                    name: uuid,
                    udf: udf.name,
                    arg,
                  });
                }}
                executeOP={makeExecuteSql(uuid)}
                language={uuid}
                getParseErr={makeGetParserErrr(uuid)}
              ></DefAndMetasView>
            ),
            <TbMathFunction></TbMathFunction>
          ),
        ];
      });
    }
  };

  const handleDrop = async () => {
    let confirm = await secondConfirm({
      title: `Drop UDF '${udf.name}'`,
      confirmText: 'Drop',
    });
    if (!confirm) {
      return;
    }
    invoke('drop_udf', { name: uuid, udf: udf.name, arg })
      .then(() => {
        showNotification('success', 'Drop udf OK');
        reload();
      })
      .catch((e) => {
        showNotification('error', 'Drop udf failure', e);
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
      <Divider />
      <MenuItem
        sx={{ color: 'error.main' }}
        onClick={() => {
          handleDrop();
        }}
      >
        <ListItemIcon>
          <DeleteForeverIcon color="error"></DeleteForeverIcon>
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
  const { setSelectTab, setTabs, tabs } = useContext(TabsSetContext);
  const { showNotification } = useContext(NotificationContext);

  const handleOpenDetails = () => {
    initMonacoLanague(uuid, DataSourceKind.Cassandra);
    const id = uuid + '-' + '-cassandra-aggregate-' + agg;
    let index = tabsHasId(tabs, id);
    if (typeof index == 'number') {
      setSelectTab(index);
    } else {
      setTabs((e) => {
        setSelectTab(tabs.length);
        return [
          ...e,
          new TabContent(
            id,
            agg,
            (
              <DefAndMetasView
                showReload
                reloadOP={async () => {
                  return await invoke<types.DefAndMetas>(
                    'cassandra_show_aggregate',
                    {
                      name: uuid,
                      aggName: agg,
                      parameters,
                    }
                  );
                }}
                executeOP={makeExecuteSql(uuid)}
                language={uuid}
                getParseErr={makeGetParserErrr(uuid)}
              ></DefAndMetasView>
            ),
            <MdFunctions></MdFunctions>
          ),
        ];
      });
    }
  };

  const handleDrop = async () => {
    let confirm = await secondConfirm({
      title: `Drop Aggregate '${agg}'`,
      confirmText: 'Drop',
    });
    if (!confirm) {
      return;
    }

    invoke('cassandra_drop_aggregate', {
      name: uuid,
      aggName: agg,
      parameters,
    })
      .then(() => {
        showNotification('success', 'Drop aggregate OK');
        reload();
      })
      .catch((e) => {
        showNotification('error', 'Drop aggregate Failure', e);
      });
  };

  return (
    <div>
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

        <Divider />
        <MenuItem
          sx={{ color: 'error.main' }}
          onClick={() => {
            handleDrop();
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon color="error"></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText>Drop</ListItemText>
        </MenuItem>
      </MenuList>
    </div>
  );
}

function CassaandraAggregatesTree() {
  const { uuid } = useContext(UuidContext);
  const idPrefix = uuid + '-cassandra-aggregate-';
  const [aggs, setAggs] = useState<types.NamedStaff[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const reload = () => {
    invoke<types.NamedStaff[]>('cassandra_list_aggregate', { name: uuid })
      .then((ts) => {
        setAggs(ts);
      })
      .catch((e) => {
        showNotification('error', 'list aggregate failed', e);
      });
  };
  const [exceed, setExceed] = useState(0);
  const matchPattern = useContext(MatchPatternContext);
  useEffect(() => {
    setExceed(aggs.filter((e) => matchPattern(e.name)).length);
  }, [aggs, matchPattern]);
  useEffect(() => {
    reload();
  }, []);
  const getItemId = (named: types.NamedStaff) => {
    return (
      idPrefix + '-' + named.name + '-' + named.cassandra_parameters.join('-')
    );
  };
  let { setUpMenu } = useContext(ContextMenuContext);

  return (
    <TreeItem
      itemId={idPrefix}
      label=<Stack
        spacing={1}
        direction="row"
        sx={{ alignItems: 'center' }}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <MenuList>
                <MenuRefresh reload={reload}></MenuRefresh>
              </MenuList>
            ),
            style: calMenuStyle(e),
          });
        }}
      >
        <MdFunctions />
        <span> aggregates </span>
      </Stack>
    >
      {aggs
        .filter((e) => matchPattern(e.name))
        .map((agg, index) => {
          if (index >= maxTreeSize) {
            return;
          }
          return (
            <TreeItem
              key={
                agg.name +
                '-' +
                (agg.cassandra_parameters
                  ? agg.cassandra_parameters.join('-')
                  : '')
              }
              itemId={getItemId(agg)}
              label=<Stack
                direction="row"
                spacing={0.5}
                sx={{ alignItems: 'center' }}
                onContextMenu={(e) => {
                  setUpMenu({
                    content: (
                      <CassandraAggregateMenu
                        reload={reload}
                        uuid={uuid}
                        agg={agg.name}
                        parameters={agg.cassandra_parameters}
                      ></CassandraAggregateMenu>
                    ),
                    style: calMenuStyle(e),
                  });
                }}
              >
                <MdFunctions></MdFunctions>
                <UdfNameAndParameters
                  udf={agg}
                  kind={DataSourceKind.Cassandra}
                ></UdfNameAndParameters>
              </Stack>
            ></TreeItem>
          );
        })}
      <ExceedMaxTreeItem exceed={exceed}></ExceedMaxTreeItem>
    </TreeItem>
  );
}

export { UdfsTree, CassaandraAggregatesTree };
