import {
  Divider,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Paper,
  Stack,
} from '@mui/material';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import { useContext, useEffect, useState } from 'react';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import InfoIcon from '@mui/icons-material/Info';
import { MdFunctions } from 'react-icons/md';
import { initMonacoLanague, tabsHasId, treeItemCommonStyle } from './Common';
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
  MatchPatternContext,
  secondConfirm,
  TabsSetContext,
  UuidContext,
} from './helper';

import { MenuRefresh } from './CommonComp';

function UdfsTree() {
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const idPrefix = uuid + '-udf-';
  const [udfs, setUdfs] = useState<types.NamedStaff[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const reload = () => {
    invoke<types.NamedStaff[]>('list_udf', { name: uuid })
      .then((ts) => {
        console.log('!!!!!!!!!!!!!!!!!', ts);
        setUdfs(ts);
      })
      .catch((e) => {
        showNotification('error', 'list udf failed', e);
      });
  };
  useEffect(() => {
    reload();
  }, []);
  const getItemId = (named: types.NamedStaff) => {
    switch (kind) {
      case DataSourceKind.Postgres:
      case DataSourceKind.Cockroach:
        return (
          idPrefix +
          '-' +
          named.name +
          '-' +
          (named.parameters ? named.parameters : 'null')
        );
      case DataSourceKind.Cassandra:
        return (
          idPrefix +
          '-' +
          named.name +
          '-' +
          named.cassandra_parameters.join('-')
        );
      default:
        return idPrefix + '-' + named.name;
    }
  };

  let { setUpMenu } = useContext(ContextMenuContext);
  const matchPattern = useContext(MatchPatternContext);
  return (
    <TreeItem
      itemId={idPrefix}
      label=<Stack
        spacing={1}
        direction="row"
        sx={{ alignItems: 'center', ...treeItemCommonStyle }}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <Paper>
                <MenuList>
                  <MenuRefresh reload={reload}></MenuRefresh>
                </MenuList>
              </Paper>
            ),
            style: calMenuStyle(e, 70),
          });
        }}
      >
        <TbMathFunction></TbMathFunction>
        <span> udfs </span>
      </Stack>
    >
      {udfs
        .filter((e) => matchPattern(e.name))
        .map((p) => (
          <TreeItem
            key={
              p.name +
              '-' +
              p.parameters +
              (p.cassandra_parameters ? p.cassandra_parameters.join('-') : '')
            }
            itemId={getItemId(p)}
            label=<Stack
              direction="row"
              spacing={1}
              sx={{ alignItems: 'center', ...treeItemCommonStyle }}
              onContextMenu={(e) => {
                setUpMenu({
                  content: (
                    <UdfMenu
                      reload={reload}
                      uuid={uuid}
                      kind={kind}
                      udf={p.name}
                      parameters={p.parameters}
                      cassandraParameters={p.cassandra_parameters}
                    ></UdfMenu>
                  ),
                  style: calMenuStyle(e, 100),
                });
              }}
            >
              <TbMathFunction></TbMathFunction>
              <span>{p.name}</span>
              {kind == DataSourceKind.Postgres ||
                (kind == DataSourceKind.Cockroach && (
                  <em> ({p.parameters}) </em>
                ))}
              {kind == DataSourceKind.Cassandra && (
                <em> ({p.cassandra_parameters.join(',')}) </em>
              )}
            </Stack>
          ></TreeItem>
        ))}
    </TreeItem>
  );
}

function UdfMenu(props: {
  uuid;
  udf;
  parameters?: string;
  cassandraParameters: string[] | null;
  reload;
  kind: DataSourceKind;
}) {
  const { uuid, udf, parameters, reload, kind, cassandraParameters } = props;
  const { setSelectTab, setTabs, tabs } = useContext(TabsSetContext);
  let arg = null;
  switch (kind) {
    case DataSourceKind.Postgres:
    case DataSourceKind.Cockroach:
      arg = { extra: 'postgres', parameters };
      break;
    case DataSourceKind.Cassandra:
      arg = { extra: 'cassandra', parameters: cassandraParameters };
      break;
  }
  const { showNotification } = useContext(NotificationContext);

  const handleOpenDetails = () => {
    initMonacoLanague(uuid, kind);
    const id = uuid + '-' + '-udf-' + udf;
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
            udf,
            (
              <DefAndMetasView
                reloadOP={async () => {
                  return await invoke<types.DefAndMetas>('show_udf', {
                    name: uuid,
                    udf,
                    arg,
                  });
                }}
                uuid={uuid}
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
      title: `Drop UDF '${udf}'`,
      confirmText: 'drop',
    });
    if (!confirm) {
      return;
    }

    invoke('drop_udf', { name: uuid, udf, arg })
      .then(() => {
        showNotification('success', 'Drop udf OK');
        reload();
      })
      .catch((e) => {
        showNotification('error', 'Drop udf Failure', e);
      });
  };

  return (
    <Paper>
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
        <Divider></Divider>
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
    </Paper>
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
                uuid={uuid}
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
      confirmText: 'drop',
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
    <Paper>
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

        <Divider></Divider>
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
    </Paper>
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
  useEffect(() => {
    reload();
  }, []);
  const getItemId = (named: types.NamedStaff) => {
    return (
      idPrefix + '-' + named.name + '-' + named.cassandra_parameters.join('-')
    );
  };
  let { setUpMenu } = useContext(ContextMenuContext);
  const matchPattern = useContext(MatchPatternContext);
  return (
    <TreeItem
      itemId={idPrefix}
      label=<Stack
        spacing={1}
        direction="row"
        sx={{ alignItems: 'center', ...treeItemCommonStyle }}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <Paper>
                <MenuList>
                  <MenuRefresh reload={reload}></MenuRefresh>
                </MenuList>
              </Paper>
            ),
            style: calMenuStyle(e, 70),
          });
        }}
      >
        <MdFunctions />
        <span> aggregates </span>
      </Stack>
    >
      {aggs
        .filter((e) => matchPattern(e.name))
        .map((p) => (
          <TreeItem
            key={
              p.name +
              '-' +
              p.parameters +
              (p.cassandra_parameters ? p.cassandra_parameters.join('-') : '')
            }
            itemId={getItemId(p)}
            label=<Stack
              direction="row"
              spacing={1}
              sx={{ alignItems: 'center', ...treeItemCommonStyle }}
              onContextMenu={(e) => {
                setUpMenu({
                  content: (
                    <CassandraAggregateMenu
                      reload={reload}
                      uuid={uuid}
                      agg={p.name}
                      parameters={p.cassandra_parameters}
                    ></CassandraAggregateMenu>
                  ),
                  style: calMenuStyle(e, 100),
                });
              }}
            >
              <MdFunctions></MdFunctions>
              <span>{p.name}</span>
              <em> ({p.cassandra_parameters.join(',')}) </em>
            </Stack>
          ></TreeItem>
        ))}
    </TreeItem>
  );
}

export { UdfsTree, CassaandraAggregatesTree };
