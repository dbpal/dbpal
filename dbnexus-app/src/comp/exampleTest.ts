import { types } from './ApiStruct.ts';
import { generateRandomString } from './helper.ts';

function testExampleRedisZset(
  length: number = 10,
  range: number = 100
): types.MuiDataTableListResp {
  var data: (string | null)[][] = [];
  for (var i = 0; i < length; i++) {
    data.push([generateRandomString(10), (Math.random() * range).toString()]);
  }
  return {
    fields: [
      { field: 'key', typ: 'string' } as types.Field,
      {
        field: 'score',
        typ: 'float64',
      } as types.Field,
    ],
    data: data,
    total: Math.floor(Math.random() * 10000 + 1000),
    sql: null,
  };
}

function testExampleRedisHash(length: number = 10): types.MuiDataTableListResp {
  var data: (string | null)[][] = [];
  for (var i = 0; i < length; i++) {
    data.push([generateRandomString(10), generateRandomString(10)]);
  }
  return {
    fields: [
      { field: 'key', typ: 'string' } as types.Field,
      {
        field: 'score',
        typ: 'float64',
      } as types.Field,
    ],
    data: data,
    total: Math.floor(Math.random() * 10000 + 1000),
    sql: null,
  };
}

function testExampleRedisSetList(
  length: number = 10
): types.MuiDataTableListResp {
  var data = [];
  for (var i = 0; i < length; i++) {
    data.push({ key: generateRandomString(10), id: generateRandomString(20) });
  }
  return {
    data: {
      data,
      fields: [{ field: 'key', typ: 'string' } as types.Field],
    },
    total: Math.floor(Math.random() * 10000 + 1000),
    sql: null,
  };
}

function testExampleQueryResp(
  fieldLength = 5,
  length: number = 10
): types.MuiDataGridData {
  var fields = [];
  for (let i = 0; i < fieldLength; i++) {
    fields.push(testExampleField());
  }
  let data = [];
  for (let i = 0; i < length; i++) {
    let d = {};
    for (let j = 0; j < fieldLength; j++) {
      d[fields[j].field as string] =
        Math.random() > 0.2 ? generateRandomString() : null;
    }
    data.push(d);
  }

  return {
    fields,
    data: data,
  };
}

function testExampleField(): types.Field {
  return {
    field: 'field' + generateRandomString(4),
    typ: 'type_' + generateRandomString(3),
    is_auto_increment: Math.random() > 0.5,
    is_primary: Math.random() > 0.5,
    nullable: Math.random() > 0.5,
    default_value: Math.random() > 0.5 ? generateRandomString() : null,
  };
}

export {
  testExampleRedisZset,
  testExampleRedisSetList,
  testExampleRedisHash,
  testExampleQueryResp,
};
