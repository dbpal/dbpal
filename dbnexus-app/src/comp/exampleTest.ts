import { types } from './ApiStruct.ts';
import { generateRandomString } from './helper.ts';

function testExampleRedisZset(
  length: number = 10,
  range: number = 100
): types.TableListResp {
  var data: (string | null)[][] = [];
  for (var i = 0; i < length; i++) {
    data.push([generateRandomString(10), (Math.random() * range).toString()]);
  }
  return {
    fields: [
      { name: 'key', typ: 'string' } as types.Field,
      {
        name: 'score',
        typ: 'float64',
      } as types.Field,
    ],
    data: data,
    total: Math.floor(Math.random() * 10000 + 1000),
    sql: null,
  };
}

function testExampleRedisHash(length: number = 10): types.TableListResp {
  var data: (string | null)[][] = [];
  for (var i = 0; i < length; i++) {
    data.push([generateRandomString(10), generateRandomString(10)]);
  }
  return {
    fields: [
      { name: 'key', typ: 'string' } as types.Field,
      {
        name: 'score',
        typ: 'float64',
      } as types.Field,
    ],
    data: data,
    total: Math.floor(Math.random() * 10000 + 1000),
    sql: null,
  };
}

function testExampleRedisSetList(length: number = 10): types.TableListResp {
  var data: (string | null)[][] = [];
  for (var i = 0; i < length; i++) {
    data.push([generateRandomString(10)]);
  }
  return {
    fields: [{ name: 'key', typ: 'string' } as types.Field],
    data: data,
    total: Math.floor(Math.random() * 10000 + 1000),
    sql: null,
  };
}

export { testExampleRedisZset, testExampleRedisSetList, testExampleRedisHash };
