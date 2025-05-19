import { types } from './ApiStruct.ts';
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
  uuid: string;
}) {
  const { reloadOP, uuid: language } = props;

  return (
    <DefAndMetasView
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
    ></DefAndMetasView>
  );
}

export { TriggerDetail };
