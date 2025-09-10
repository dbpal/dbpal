import { types } from './ApiStruct.ts';
import { ErrAndLoc } from './helper.ts';
import { DefAndMetasView } from './UdfAndMetaView.tsx';

function triggerDetailsToMetas(x: types.TriggerDetail): types.MetaValue[] {
  const getEnableStr = (): string => {
    return x.enable.enable + '';
  };

  const getEnableDesc = (): string => {
    if (x.enable.kind == 'postgres') {
      return `Controls in which session_replication_role modes the trigger fires. O = trigger fires in
      “origin” and “local” modes, D = trigger is disabled, R = trigger fires in “replica” mode, A
      = trigger fires always.`;
    }
    return '';
  };
  const ret: types.MetaValue[] = [
    // todo timing can be object
    { name: 'timming', value: x.timing as string, desc: '' },
    { name: 'event', value: x.event, desc: '' },
  ];
  if (x.enable) {
    ret.push({
      name: 'enable',
      value: getEnableStr(),
      desc: getEnableDesc(),
    });
  }
  return ret;
}

function TriggerDetail(props: {
  reloadOP: () => Promise<types.TriggerDetail>;
  executeOP: (sql: string) => Promise<any>;
  uuid: string;
  getParseErr: (sql: string) => Promise<ErrAndLoc | null>;
}) {
  const { reloadOP, uuid: language, executeOP, getParseErr } = props;

  return (
    <DefAndMetasView
      heightName="trigger-detail"
      language={language}
      reloadOP={async () => {
        return reloadOP().then((detail) => {
          return {
            def: detail.sql,
            metas: [...triggerDetailsToMetas(detail), ...detail.metas],
            detail,
          };
        });
      }}
      getParseErr={getParseErr}
      executeOP={executeOP}
    ></DefAndMetasView>
  );
}

export { TriggerDetail };
