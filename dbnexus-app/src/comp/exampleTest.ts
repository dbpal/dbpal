import { types } from './ApiStruct.ts';
import { stringToHexArray, arrayToHex } from './common.tsx';
import { generateRandomString, generateRandomUint8Array } from './helper.ts';

function testExampleRedisZset(
  length: number = 10,
  range: number = 100
): types.MuiDataTableListResp {
  var data = [];
  for (var i = 0; i < length; i++) {
    data.push({
      key: testRedisKeyOrValue(),
      score: (Math.random() * range).toString(),
    });
  }
  return {
    data: {
      fields: [
        { field: 'key', typ: 'string' } as types.Field,
        {
          field: 'score',
          typ: 'float64',
        } as types.Field,
      ],
      data: data,
    },
    total: Math.floor(Math.random() * 10000 + 1000),
    sql: null,
  };
}

function testRedisKeyOrValue(): string {
  if (Math.random() > 0.5) {
    return arrayToHex(stringToHexArray(generateRandomString()));
  } else {
    return arrayToHex(generateRandomUint8Array());
  }
}

function testExampleRedisHash(length: number = 10): types.MuiDataTableListResp {
  var data = [];
  for (var i = 0; i < length; i++) {
    data.push({
      key: testRedisKeyOrValue(),
      value: testRedisKeyOrValue(),
    });
  }
  return {
    data: {
      fields: [
        { field: 'key', typ: 'string' } as types.Field,
        {
          field: 'value',
          typ: 'string',
        } as types.Field,
      ],
      data,
    },
    total: Math.floor(Math.random() * 10000 + 1000),
    sql: null,
  };
}

function testExampleRedisList(length: number = 10): types.MuiDataTableListResp {
  var data = [];
  for (var i = 0; i < length; i++) {
    data.push({ value: testRedisKeyOrValue(), seq: i.toString() });
  }
  return {
    data: {
      data,
      fields: [
        { field: 'seq', typ: 'int' } as types.Field,
        { field: 'value', typ: 'string' } as types.Field,
      ],
    },
    total: Math.floor(Math.random() * 10000 + 1000),
    sql: null,
  };
}

function testExampleRedisSet(length: number = 10): types.MuiDataTableListResp {
  var data = [];
  for (var i = 0; i < length; i++) {
    data.push({ value: testRedisKeyOrValue() });
  }
  return {
    data: {
      data,
      fields: [{ field: 'value', typ: 'string' } as types.Field],
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
    field: 'field_' + generateRandomString(4),
    typ: 'type_' + generateRandomString(3),
    is_auto_increment: Math.random() > 0.5,
    is_primary: Math.random() > 0.5,
    nullable: Math.random() > 0.5,
    default_value: Math.random() > 0.5 ? generateRandomString() : null,
  };
}

export {
  testExampleRedisZset,
  testExampleRedisSet,
  testExampleRedisHash,
  testExampleQueryResp,
  testExampleRedisList,
};
