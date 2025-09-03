import {
  Box,
  Button,
  FormControlLabel,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Stack,
  Switch,
} from '@mui/material';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import { useContext, useEffect, useState } from 'react';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import InfoIcon from '@mui/icons-material/Info';
import {
  DefAndMetas,
  EscDownEvent,
  initMonacoLanague,
  modalStyle,
  tabsHasId,
} from './Common';
import types from './ApiStruct';
import { invoke } from '@tauri-apps/api/core';
import { NotificationContext } from './Notification';
import { TabContent } from './Tabs';
import { DefAndMetasView } from './UdfAndMetaView';
import { GiProgression } from 'react-icons/gi';
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
  UuidContext,
} from './helper';

import { ExceedMaxTreeItem, MenuRefresh } from './CommonComp';
function SequenceTree() {
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const idPrefix = uuid + '-sequence-';
  const [seqs, setSeqs] = useState<types.SequenceDetail[]>([]);
  const { showNotification } = useContext(NotificationContext);

  const reload = () => {
    invoke<types.SequenceDetail[]>('list_sequences', { name: uuid })
      .then((ts) => {
        setSeqs(ts);
      })
      .catch((e) => {
        showNotification('error', 'list sequences failed', e);
      });
  };

  const [exceed, setExceed] = useState(0);
  const matchPattern = useContext(MatchPatternContext);
  useEffect(() => {
    setExceed(seqs.filter((e) => matchPattern(e.name)).length);
  }, [seqs, matchPattern]);
  const getItemId = (x: types.SequenceDetail) => {
    switch (kind) {
      default:
        return idPrefix + '-' + x.name;
    }
  };

  let { setUpMenu } = useContext(ContextMenuContext);
  useEffect(() => {
    reload();
  }, []);

  return (
    <TreeItem
      itemId={idPrefix}
      label=<Stack
        direction="row"
        spacing={1}
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
        <GiProgression></GiProgression>
        <span> sequences </span>
      </Stack>
    >
      {seqs
        .filter((e) => matchPattern(e.name))
        .map((seq, index) => {
          if (index >= maxTreeSize) {
            return;
          }
          return (
            <TreeItem
              key={seq.name}
              itemId={getItemId(seq)}
              style={{ alignItems: 'center' }}
              label=<Stack
                spacing={1}
                direction="row"
                onContextMenu={(e) => {
                  setUpMenu({
                    content: (
                      <SequenceMenu
                        reload={reload}
                        uuid={uuid}
                        kind={kind}
                        seq={seq}
                      ></SequenceMenu>
                    ),
                    style: calMenuStyle(e),
                  });
                }}
                sx={{ alignItems: 'center' }}
              >
                <GiProgression></GiProgression>
                <span>{seq.name}</span>
              </Stack>
            ></TreeItem>
          );
        })}
      <ExceedMaxTreeItem exceed={exceed}></ExceedMaxTreeItem>
    </TreeItem>
  );
}

function details2DefMetas(d: types.SequenceDetail): DefAndMetas {
  return {
    def: d.sql,
    metas: [
      { name: 'data type', value: d.data_type },
      { name: 'start_value', value: d.start_value },
      { name: 'min value', value: d.min_value },
      { name: 'max value', value: d.max_value },
      { name: 'increment by', value: d.increment_by },
      { name: 'last value', value: d.last_value },
    ],
  };
}

function SequenceMenu(props: {
  uuid;
  seq: types.SequenceDetail;
  reload;
  kind: DataSourceKind;
}) {
  const { uuid, seq, reload, kind } = props;
  const { setSelectTab, setTabs, tabs } = useContext(TabsSetContext);
  const { showNotification } = useContext(NotificationContext);
  const [showDrop, setShowDrop] = useState(false);
  const [cascade, setCascade] = useState(false);
  const supportCaacase = [DataSourceKind.Postgres].includes(kind);

  const handleOpenDetails = () => {
    initMonacoLanague(uuid, kind);

    const id = uuid + '-' + '-sequence-' + seq.name;
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
            seq.name,
            (
              <DefAndMetasView
                reloadOP={async () => {
                  return Promise.resolve(details2DefMetas(seq));
                }}
                executeOP={makeExecuteSql(uuid)}
                language={uuid}
                getParseErr={makeGetParserErrr(uuid)}
              ></DefAndMetasView>
            ),
            <GiProgression></GiProgression>
          ),
        ];
      });
    }
  };

  const handleDrop = async () => {
    let confirm = await secondConfirm({
      title: `Drop Sequence '${seq.name}'`,
      confirmText: 'Drop',
    });
    if (!confirm) {
      return;
    }
    invoke('drop_sequence', { name: uuid, seq: seq.name, cascade })
      .then(() => {
        showNotification('success', 'Drop sequence OK');
        reload();
        setShowDrop(false);
      })
      .catch((e) => {
        showNotification('error', 'Drop sequence Failure', e);
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

        <MenuItem
          sx={{ color: 'error.main' }}
          onClick={() => {
            setShowDrop(true);
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon color="error"></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText> Drop</ListItemText>
        </MenuItem>
      </MenuList>
      {showDrop && (
        <Modal
          open={showDrop}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setShowDrop(false);
          }}
        >
          <Box sx={modalStyle} width="45%">
            <Stack spacing={1}>
              {supportCaacase && (
                <Stack>
                  <FormControlLabel
                    control={
                      <Switch
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
                  variant="contained"
                  color="error"
                >
                  Drop
                </Button>
              </Stack>
            </Stack>
          </Box>
        </Modal>
      )}
    </div>
  );
}

export { SequenceTree };
