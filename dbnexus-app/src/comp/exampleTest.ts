import { types } from './ApiStruct.ts';
import { DocsAndCount } from './Common.tsx';
import { arrayToHex, generateRandomString, generateRandomUint8Array, stringToHexArray } from './helper.ts';
import { BSON } from 'bson';

function testRedisZset(
  length: number = 10,
  range: number = 100
): types.MuiDataTableListResp {
  let data = [];
  for (let i = 0; i < length; i++) {
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

function testRedisHash(length: number = 10): types.MuiDataTableListResp {
  let data = [];
  for (let i = 0; i < length; i++) {
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

function testRedisList(length: number = 10): types.MuiDataTableListResp {
  let data = [];
  for (let i = 0; i < length; i++) {
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

function testRedisSet(length: number = 10): types.MuiDataTableListResp {
  let data = [];
  for (let i = 0; i < length; i++) {
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

function testBsonDoc(): BSON.Document {
  const bsonDocument = {
    _id: new BSON.ObjectId(),
    string: generateRandomString(10),
    int32: new BSON.Int32(Math.random() * -100),
    long: new BSON.Long(100),
    float: Math.random() * 100,
    boolean: Math.random() > 0.5,
    null: null,
    timestamp: new BSON.Timestamp(new BSON.Long(1732158103)),
    date: new Date(),
    decimal: new BSON.Decimal128('10.5'),
    regex: /haha/i,
    array: [1, 2, 3, 'four', { key: 'value' }],
    code: new BSON.Code('function() { return 42; }'),
    binary: new BSON.Binary([1, 2, 3, 4]),

    float32_array: new Float32Array([
      Math.random() * 10,
      Math.random() * 100,
      Math.random() * 1000,
    ]),
    int8_array: new Int8Array([
      Math.random() * 10,
      Math.random() * 100,
      Math.random() * 1000,
    ]),
  };
  return bsonDocument;
}

function testBsonDocTableList(): BSON.Document {
  const bsonDocument = {
    _id: new BSON.ObjectId().toHexString(),
    string: generateRandomString(),
    int32: Math.floor(Math.random() * 100),
    long: Math.floor(Math.random() * 100),
    float: Math.random() * 100,
    boolean: Math.random() > 0.5,
    null: null,
    timestamp: Math.floor(Math.random() * 1732158103),
    date: new Date().toISOString(),
    decimal: 10.5,
    regex: '/haha/i',
    array: [1, 2, 3, 'four', { key: 'value' }],
    code: 'function() { return 42; }',
    binary: [1, 2, 3, 4],
  };
  return bsonDocument;
}

function testMongoTableList(length: number = 10): types.MuiDataTableListResp {
  let fields = [
    { field: '_id' } as types.Field,
    { field: 'string' } as types.Field,
    { field: 'int32' } as types.Field,
    { field: 'long' } as types.Field,
    { field: 'boolean' } as types.Field,
    { field: 'null' } as types.Field,
    { field: 'timestamp' } as types.Field,
    { field: 'date' } as types.Field,
    { field: 'decimal' } as types.Field,
    { field: 'regex' } as types.Field,
    { field: 'array' } as types.Field,
    { field: 'code' } as types.Field,
    { field: 'binary' } as types.Field,
  ];

  let data = [];
  for (let i = 0; i < length; i++) {
    data.push(testBsonDocTableList());
  }
  return {
    data: {
      fields: fields,
      data,
    },
    sql: null,
    total: Math.ceil(Math.random() * length * 10) + 100,
  };
}

function testDocsAndCount(length: number = 10): DocsAndCount {
  let docs = [];
  for (let i = 0; i < length; i++) {
    docs.push(testBsonDoc());
  }
  return new DocsAndCount(docs, Math.ceil(Math.random() * 10 * 10 + length));
}

function testQueryResp(
  fieldLength = 5,
  length: number = 10
): types.MuiDataGridData {
  let fields = [];
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

function testQueryRespTableList(
  fieldLength = 5,
  length: number = 10
): types.MuiDataTableListResp {
  return {
    data: testQueryResp(fieldLength, length),
    total: Math.ceil(Math.random() * 1000),
    sql: 'select * from ' + generateRandomString(4),
  };
}

function testExampleField(): types.Field {
  const someSqlTypes = [
    'INT',
    'FLOAT',
    'DOUBLE',
    'DECIMAL',
    'CHAR',
    'VARCHAR',
    'TEXT',
    'DATE',
    'TIME',
    'DATETIME',
    'TIMESTAMP',
    'BOOLEAN',
    'BLOB',
    'ENUM',
    'SET',
  ];

  return {
    field: 'field_' + generateRandomString(4),
    typ: someSqlTypes[Math.floor(Math.random() * someSqlTypes.length)],
    is_primary: Math.random() > 0.5,
    is_auto_increment: Math.random() > 0.8,
    nullable: Math.random() > 0.8,
    default_value: Math.random() > 0.5 ? generateRandomString() : null,
    sortable: true,
    firebird_binary_sub_type: null
  };
}

/*
  multi resp have this rust structure
[
  {
    "Err": "this a error"
  },
  {
    "Ok": {
      "fields": [],
      "data": []
    }
  }
]
*/
function testMultiQueryResp(maxResult: number = 3) {
  let ret = [];
  for (let i = 0; i < Math.ceil(Math.random() * maxResult); i++) {
    if (Math.random() > 0.66) {
      ret.push({ Err: `This is random error.` });
    } else {
      ret.push({ Ok: testQueryResp() });
    }
  }
  return ret;
}

function testMetaValues(): types.MetaValue[] {
  return [
    { name: 'version', value: '10.0.0', desc: 'the version of the server.' },
    { name: 'haha', value: 'heihei', desc: '' },
    { name: 'extra', value: generateRandomString(10), desc: '' },
  ];
}
function testQueryLogs(length: number = 20): types.QueryLogEvent[] {
  let ret: types.QueryLogEvent[] = [];
  const randomString = (length: number = 10) => {
    let ret = '';
    for (let i = 0; i < length; i++) {
      ret += generateRandomString(Math.random() * 20);
      ret += '\n';
    }
    return ret;
  };
  for (let i = 0; i < length; i++) {
    ret.push({
      time: Date.now() - Math.ceil(Math.random() * 1000000000000),
      success: Math.random() > 0.3,
      sql: randomString(30),
    });
  }
  return ret;
}

function testTriggerDetail(): types.TriggerDetail {
  function testEvent(): types.TriggerEvent {
    const a = ['Insert', 'Update', 'Delete'];
    let index = Math.ceil(Math.random() * a.length);
    //@ts-ignore
    return a[index];
  }
  function testTiming(): types.TriggerTiming {
    const a = ['Before', 'After', 'InsteadOf'];
    let index = Math.ceil(Math.random() * a.length);
    //@ts-ignore
    return a[index];
  }
  return {
    name: 'xxxx',
    event: testEvent(),
    timing: testTiming(),
    enable: { kind: 'postgres', enable: 'o' },
    sql: `DELIMITER //

CREATE TRIGGER after_employee_insert
AFTER INSERT ON employees
FOR EACH ROW
BEGIN
    INSERT INTO audit_log (action, employee_id)
    VALUES ('INSERT', NEW.id);
END;

//

DELIMITER ;`,
    metas: testMetaValues(),
  };
}

function testProcedure(): types.ProcedureDetail {
  return {
    def: `DELIMITER //

CREATE PROCEDURE GetUserDetails (
    IN userId INT
)
BEGIN
    SELECT 
        id, 
        username, 
        email, 
        created_at 
    FROM 
        users 
    WHERE 
        id = userId;
END //

DELIMITER ;`,
    metas: testMetaValues(),
  };
}

function testUDF(): types.DefAndMetas {
  return {
    def: `CREATE FUNCTION AddNumbers (
    a INT,
    b INT
)
RETURNS INT
BEGIN
    DECLARE result INT;
    SET result = a + b;
    RETURN result;
END;`,
    metas: testMetaValues(),
  };
}
export {
  testRedisZset,
  testRedisSet,
  testRedisHash,
  testQueryResp,
  testRedisList,
  testMultiQueryResp,
  testMetaValues,
  testBsonDoc,
  testBsonDocTableList,
  testMongoTableList,
  testDocsAndCount,
  testQueryLogs,
  testQueryRespTableList,
  testTriggerDetail,
  testUDF,
  testProcedure,
};
