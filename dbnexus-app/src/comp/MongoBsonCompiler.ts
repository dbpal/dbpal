import { tokenize } from 'esprima';
import {
  BSON,
  Int32,
  Long,
  ObjectId,
  Timestamp,
  Code,
  Decimal128,
  Binary,
  Double,
  Document,
} from 'bson';

class MongoBsonObject {
  constructor(
    public eles: MongoBsonKV[],
    public startLoc?: Location,
    public endLoc?: Location
  ) { }
  format(tableSize: number = 2): string {
    return new MongoBsonValue(MongoBsonValueKind.Object, this).format(
      1,
      true,
      tableSize
    );
  }

  compile(): Document {
    let ret = {};
    for (let i = 0; i < this.eles.length; i++) {
      let kv = this.eles[i];
      const key = trimQuoteMark(this.eles[i].key.value);
      if (key in ret) {
        throw new MongoBsonError(`key '${key}' already exits.`, kv.key.loc);
      }
      ret[key] = kv.value.compile();
    }
    return ret;
  }
}

function trimQuoteMark(str: string): string {
  if (str[0] == '"' || str[0] == "'" || str[0] == '`') {
    return str.substring(1, str.length - 1);
  } else {
    return str;
  }
}

class MongoBsonKV {
  constructor(public key: Token, public value: MongoBsonValue) { }
}

class MongoBsonAs {
  constructor(
    public value: MongoBsonValue,
    public kind: Token,
    public extra?: MongoBsonBinaryAllSubType
  ) { }
  compile(): any {
    switch (this.kind.value.toLocaleLowerCase()) {
      case 'int':
        return this.compileInt32();
      case 'long':
        return this.compileLong();
      case 'timestamp':
        return this.compileTimestamp();
      case 'date':
        return this.compileDate();
      case 'decimal':
        return this.compileDecimal();
      case 'code':
        return this.compileJavascript();
      case 'binary':
        return this.compileBinary();
      case 'objectid':
        return this.compileObjectID();

      default:
        throw new MongoBsonError(
          `unknown convert named '${this.kind.value}'`,
          this.kind.loc
        );
    }
  }

  compileBinary(): Binary {
    let v = this.value.compile();
    let ty = typeof v;

    let subType = this.extra
      ? this.extra.toBinarySubTypeNumber()
      : Binary.SUBTYPE_DEFAULT;
    if (ty === 'string') {
      return new Binary(v, subType);
    }
    if (ty === 'object' && v instanceof Array && this.extra.isFloat32()) {
      return Binary.fromFloat32Array(new Float32Array(v));
    }
    if (ty === 'object' && v instanceof Array && this.extra.isInt8()) {
      return Binary.fromInt8Array(new Int8Array(v));
    }
    if (ty === 'object' && v instanceof Array && subType == Binary.SUBTYPE_USER_DEFINED) {
      return new Binary(new Uint8Array(v), parseInt(this.extra.extra[0].value))
    }
    if (v instanceof Array) {
      return new Binary(new Uint8Array(v), subType)
    }
    this.cannotConvert(ty, 'binary');
  }

  compileJavascript(): Code {
    let v = this.value.compile();
    let ty = typeof v;
    if (ty === 'string') {
      return new Code(v);
    }
    this.cannotConvert(ty, 'code');
  }

  compileDecimal(): Decimal128 {
    let v = this.value.compile();
    let ty = typeof v;
    if (ty === 'string') {
      return new Decimal128(v);
    }
    if (ty === 'number') {
      return new Decimal128(v.toString());
    }
    this.cannotConvert(ty, 'decimal128');
  }

  compileDate(): Date {
    let v = this.value.compile();
    let ty = typeof v;
    if (ty === 'string') {
      let x = new Date(v);
      if (x instanceof Date) {
        return x
      }
      throw new MongoBsonError(`not a valid date string:${v}`, this.kind.loc);
    }
    this.cannotConvert(ty, 'date');
  }
  compileObjectID(): ObjectId {
    let v = this.value.compile();
    let ty = typeof v;
    if (ty === 'string') {
      return new ObjectId(v as string);
    }
    if (ty === 'object' && v instanceof Array) {
      // TODO maybe more check
      return new ObjectId(new Uint8Array(v as number[]));
    }
    this.cannotConvert(ty, 'ObjectID');
  }

  cannotConvert(ty: string, to: string): never {
    throw new MongoBsonError(`can't compile '${ty}' to ${to}`, this.kind.loc);
  }
  compileInt32(): Int32 {
    let v = this.value.compile();
    let ty = typeof v;
    if (ty === 'number') {
      return new Int32(v);
    }
    this.cannotConvert(ty, 'int');
  }
  compileLong(): Long {
    let v = this.value.compile();
    let ty = typeof v;
    if (ty === 'number') {
      return new Long(v);
    }
    if (ty === 'string') {
      return new Long(v);
    }
    this.cannotConvert(ty, 'long');
  }
  compileTimestamp(): Timestamp {
    let v = this.value.compile();
    let ty = typeof v;
    if (ty === 'bigint') {
      return new Timestamp(v);
    }
    if (ty === 'string') {
      return new Timestamp(new Long(v));
    }
    if (ty === 'number') {
      return new Timestamp(new Long(v));
    }
    this.cannotConvert(ty, 'timestamp');
  }
}

enum MongoBsonValueKind {
  Number = 'number',
  String = 'string',
  As = 'as',
  Array = 'array',
  Object = 'object',
  RegExp = 'regexp',
  Null = 'null',
  Undefined = 'undefined',
  Int32 = 'int32',
  Long = 'long',
  Code = 'code',
  Timestamp = 'timestamp',
  Date = 'date',
  ObjectID = 'objectid',
  Binary = 'binary',
  Decimal = 'decimal',
  Boolean = 'boolean',
  Float32Array = 'float32_array',
  Int8Array = 'int8_array',
}

class MongoBsonBinaryAllSubType {
  constructor(public kind: Token, public extra?: Token[]) { }

  static Generic = 'generic';
  static Function = 'function';
  static ByteArray = 'byte_array';
  static Uuid = 'uuid';
  static Md5 = 'md5';
  static Encrypted = 'encrypted';
  static Column = 'column';
  static Sensitive = 'sensitive';
  static Vector = 'vector';
  static UserDefine = 'user_define';
  extraIndex(index: number): Token | undefined {
    if (!this.extra) {
      return undefined;
    }
    if (index >= this.extra.length) {
      return undefined;
    }
    return this.extra[index];
  }
  isFloat32(): boolean {
    let extra = this.extraIndex(0);
    return extra && extra.value.toLowerCase() == 'float32';
  }

  isInt8(): boolean {
    let extra = this.extraIndex(0);
    return extra && extra.value.toLowerCase() == 'int8';
  }

  static allValues(): string[] {
    return [
      MongoBsonBinaryAllSubType.Generic,
      MongoBsonBinaryAllSubType.Function,
      MongoBsonBinaryAllSubType.ByteArray,
      MongoBsonBinaryAllSubType.Uuid,
      MongoBsonBinaryAllSubType.Md5,
      MongoBsonBinaryAllSubType.Encrypted,
      MongoBsonBinaryAllSubType.Column,
      MongoBsonBinaryAllSubType.Sensitive,
      MongoBsonBinaryAllSubType.Vector,
      MongoBsonBinaryAllSubType.UserDefine,
    ];
  }

  toBinarySubTypeNumber(): number | undefined {
    let lower = this.kind.value.toLowerCase();
    if (lower == MongoBsonBinaryAllSubType.Generic) {
      return Binary.SUBTYPE_DEFAULT;
    }
    if (lower == MongoBsonBinaryAllSubType.Function) {
      return Binary.SUBTYPE_FUNCTION;
    }
    if (lower == MongoBsonBinaryAllSubType.ByteArray) {
      return Binary.SUBTYPE_BYTE_ARRAY;
    }
    if (lower == MongoBsonBinaryAllSubType.Uuid) {
      return Binary.SUBTYPE_UUID;
    }
    if (lower == MongoBsonBinaryAllSubType.Md5) {
      return Binary.SUBTYPE_MD5;
    }
    if (lower == MongoBsonBinaryAllSubType.Encrypted) {
      return Binary.SUBTYPE_ENCRYPTED;
    }
    if (lower == MongoBsonBinaryAllSubType.Column) {
      return Binary.SUBTYPE_COLUMN;
    }
    if (lower == MongoBsonBinaryAllSubType.Sensitive) {
      return Binary.SUBTYPE_SENSITIVE;
    }
    if (lower == MongoBsonBinaryAllSubType.Vector) {
      return Binary.SUBTYPE_VECTOR;
    }
    if (lower == MongoBsonBinaryAllSubType.UserDefine) {
      return Binary.SUBTYPE_USER_DEFINED;
    }
    return undefined;
  }

  static fromNumber(x: number): string {
    switch (x) {
      case Binary.SUBTYPE_DEFAULT:
        return MongoBsonBinaryAllSubType.Generic;
      case Binary.SUBTYPE_FUNCTION:
        return MongoBsonBinaryAllSubType.Function;
      case Binary.SUBTYPE_BYTE_ARRAY:
        return MongoBsonBinaryAllSubType.ByteArray;
      case Binary.SUBTYPE_UUID:
        return MongoBsonBinaryAllSubType.Uuid;
      case Binary.SUBTYPE_ENCRYPTED:
        return MongoBsonBinaryAllSubType.Encrypted;
      case Binary.SUBTYPE_COLUMN:
        return MongoBsonBinaryAllSubType.Column;
      case Binary.SUBTYPE_SENSITIVE:
        return MongoBsonBinaryAllSubType.Sensitive;
      case Binary.SUBTYPE_USER_DEFINED:
        return MongoBsonBinaryAllSubType.UserDefine;
      default:
        return `${MongoBsonBinaryAllSubType.UserDefine} ${x}`;
    }
  }
}

class MongoBsonValue {
  constructor(
    public kind: MongoBsonValueKind,
    public value:
      | Token
      | MongoBsonObject
      | MongoBsonAs
      | MongoBsonArray
      | null
      | undefined
      | Binary
      | Float32Array
      | Int8Array
  ) { }

  asToken(): Token {
    return this.value as Token;
  }
  compile(): any {
    switch (this.kind) {
      case MongoBsonValueKind.Boolean:
        return this.asToken().value.toLocaleUpperCase() === 'true';
      case MongoBsonValueKind.Number:
        return parseFloat(this.asToken().value);
      case MongoBsonValueKind.String:
        return trimQuoteMark(this.asToken().value);
      case MongoBsonValueKind.As:
        return (this.value as MongoBsonAs).compile();
      case MongoBsonValueKind.Array:
        return (this.value as MongoBsonArray).compile();
      case MongoBsonValueKind.Object:
        return (this.value as MongoBsonObject).compile();
      case MongoBsonValueKind.RegExp:
        return eval(this.asToken().value);
      case MongoBsonValueKind.Null:
        return null;
      case MongoBsonValueKind.Undefined:
        return undefined;
      default:
        throw new MongoBsonError(`this kind '${this.kind}' should in ast`);
    }
  }

  formatArray(
    x: any[],
    newLine: boolean,
    tabSize: number,
    ident: number
  ): string {
    let ret: string;
    ret = '[';
    newLine = newLine || this.elementsSize() > 100;
    if (newLine) {
      ret += '\n';
    } else {
      ret += ' ';
    }
    x.forEach((e, index) => {
      ret += MongoBsonValue.doIdent(newLine, tabSize, ident);
      let ety = typeof e;
      if (ety == 'number') {
        ret += e;
      } else {
        ret += e.format(ident + 1, false, tabSize);
      }

      if (index != x.length - 1) {
        ret += ',';
      }
      if (newLine) {
        ret += '\n';
      } else {
        ret += ' ';
      }
    });
    ret += ']';
    if (newLine) {
      ret += '\n';
    }
    return ret;
  }
  static doIdent(newLine: boolean, tabSize: number, ident: number) {
    if (newLine) {
      return ' '.repeat(tabSize).repeat(ident);
    } else {
      return '';
    }
  }
  format(ident: number, newLine: boolean, tabSize: number): string {
    let ret = '';
    switch (this.kind) {
      case MongoBsonValueKind.Float32Array:
        return (
          this.formatArray(
            Array.from(this.value as Float32Array),
            newLine,
            tabSize,
            ident
          ) + ' as binary vector float32'
        );
      case MongoBsonValueKind.Int8Array:
        return (
          this.formatArray(
            Array.from(this.value as Int8Array),
            newLine,
            tabSize,
            ident
          ) + ' as binary vector int8'
        );
      case MongoBsonValueKind.Int32:
        return `${this.asToken().value} as int`;
      case MongoBsonValueKind.Long:
        return `${this.asToken().value} as long`;
      case MongoBsonValueKind.Null:
        return `null`;
      case MongoBsonValueKind.Undefined:
        return `undefined`;
      case MongoBsonValueKind.ObjectID:
        return `'${this.asToken().value}' as ObjectID`;
      case MongoBsonValueKind.Binary:
        let binaryValue = this.value as Binary;
        ret += '[';
        if (newLine) {
          ret += '\n';
        } else {
          ret += ' ';
        }
        for (let i = 0; i < binaryValue.buffer.length; i++) {
          ret += binaryValue.buffer[i].toString();
          if (i != binaryValue.buffer.length - 1) {
            ret += ',';
          }
          if (newLine) {
            ret += '\n';
          } else {
            ret += ' ';
          }
        }
        ret += ']';
        // here is type annotion
        ret +=
          ' as binary ' +
          MongoBsonBinaryAllSubType.fromNumber(binaryValue.sub_type);
        binaryValue.sub_type;
        return ret;
      case MongoBsonValueKind.Date:
        return `'${this.asToken().value}' as date`;
      case MongoBsonValueKind.Timestamp:
        return `${this.asToken().value} as timestamp`;
      case MongoBsonValueKind.Code:
        return this.asToken().value + ' as code';
      case MongoBsonValueKind.Number:
        return `${this.asToken().value}`;
      case MongoBsonValueKind.Boolean:
        return this.asToken().value;
      case MongoBsonValueKind.RegExp:
        return `${this.asToken().value}`; // this is regexp
      case MongoBsonValueKind.String:
        let x = this.asToken().value;
        return x;
      case MongoBsonValueKind.Decimal:
        return `'${this.asToken().value}' as decimal`;

      case MongoBsonValueKind.As:
        let as = this.value as MongoBsonAs;
        ret = as.value.format(ident, false, tabSize);
        ret += ` as ${as.kind.value}`;
        if (as.extra) {
          ret += ` ${as.extra.kind.value}`;
          if (as.extra.extra) {
            as.extra.extra.forEach((e) => {
              ret += ` ${e.value}`;
            });
          }
        }
        return ret;
      case MongoBsonValueKind.Array:
        let valueArr = this.value as MongoBsonArray;
        return this.formatArray(valueArr.eles, newLine, tabSize, ident);
      case MongoBsonValueKind.Object:
        let valueObj = this.value as MongoBsonObject;
        ret = '{';
        newLine = newLine || this.elementsSize() > 100;
        if (newLine) {
          ret += '\n';
        } else {
          ret += ' ';
        }
        valueObj.eles.forEach((e, index) => {
          ret += MongoBsonValue.doIdent(newLine, tabSize, ident);
          ret += e.key.value + ': ';
          if (e.value == null) {
            ret += 'null';
          } else if (e.value == undefined) {
            ret += 'undefined';
          } else {
            ret += e.value.format(ident + 1, false, tabSize);
          }
          if (index != valueObj.eles.length - 1) {
            ret += ',';
          }

          if (newLine) {
            ret += '\n';
          } else {
            ret += ' ';
          }
        });

        ret += '}';
        if (newLine) {
          ret += '\n';
        }

        return ret;
      default:
        throw new MongoBsonError(`unhandled type '${this.kind}' of format`);
    }
  }

  elementsSize(): number {
    let count = 0;
    switch (this.kind) {
      case MongoBsonValueKind.Number:
      case MongoBsonValueKind.String:
        return this.asToken().value.length;
      case MongoBsonValueKind.Array:
        (this.value as MongoBsonArray).eles.forEach((e) => {
          count += e.elementsSize();
        });
        return count;
      case MongoBsonValueKind.Object:
        (this.value as MongoBsonObject).eles.forEach((e) => {
          count += e.value.elementsSize();
          count += e.key.value.length;
        });
        return count;
      case MongoBsonValueKind.As:
        let as = this.value as MongoBsonAs;
        count += as.kind.value.length;
        count += as.value.elementsSize();
        return count;
    }
  }
}

class MongoBsonArray {
  constructor(
    public eles: MongoBsonValue[],
    public startLoc?: Location,
    public endLoc?: Location
  ) { }

  compile(): any {
    return this.eles.map((e) => e.compile());
  }
}

function parseMongoBson(input: string): MongoBsonObject {
  let tokens = tokenize(input, { loc: true }) as Token[];
  let parser = new Parser(tokens);
  let x = parser.parseObject();
  if (!parser.tokenAllConsumed()) {
    let token = parser.peek();
    throw new MongoBsonError(`expect EOF but '${token.value}'`, token.loc);
  }
  return x;
}

function formatMongoBsonStr(input: string, tabSize: number): string {
  let ast = parseMongoBson(input);
  let s = new MongoBsonValue(MongoBsonValueKind.Object, ast).format(
    1,
    true,
    tabSize
  );
  return s;
}

interface Token {
  type?: string;
  value: string;
  loc?: Location;
}

class MongoBsonError extends Error {
  msgWithLoc: string;
  constructor(public msg: string, public loc?: Location) {
    let msgWithLoc = MongoBsonError.makeMsg(msg, loc);
    super(msgWithLoc);
    this.msgWithLoc = msgWithLoc;
  }

  static middleColumn(loc?: Location): number {
    return Math.floor((loc.start.column + loc.end.column) / 2) + 1;
  }

  static makeMsg(msg: string, loc?: Location): string {
    if (loc) {
      return `${loc.start.line}:${loc.start.column + 1} ${msg}`;
    } else {
      return `${msg}`;
    }
  }

  toString(): string {
    return this.msgWithLoc;
  }
}

class Parser {
  index: number = 0;
  constructor(public tokens: Token[]) { }

  tokenAllConsumed(): boolean {
    return this.index >= this.tokens.length;
  }
  parseObject(): MongoBsonObject {
    let tokenStart = this.peek();
    let startLoc = tokenStart.loc;
    this.except('{');
    let defs = [];
    while (this.peek().value != '}') {
      let def = this.parseObjectDefs();
      defs.push(def);
      if (this.peek().value == ',') {
        this.next();
      }
    }
    let tokenEnd = this.peek();
    let endLoc = tokenEnd.loc;
    this.except('}');
    return new MongoBsonObject(defs, endLoc, startLoc);
  }

  parseObjectDefs(): MongoBsonKV {
    let token = this.peek();
    if (token.type != 'Identifier' && token.type != 'String') {
      throw new MongoBsonError(
        `expect identifer or string,but '${token.value}'`,
        token.loc
      );
    }
    this.next();
    let tokenColon = this.peek();
    if (tokenColon.value != ':') {
      throw new MongoBsonError(
        `expect colon,but '${tokenColon.value}'`,
        tokenColon.loc
      );
    }
    this.next();
    // here is value
    let value = this.parseValueAs();

    return new MongoBsonKV(token, value);
  }

  parseValueAs(): MongoBsonValue {
    let value = this.parseValue();
    let token = this.peek();
    if (token.value != 'as') {
      return value;
    } else {
      this.next();
      token = this.peek();
      this.next();
      let extra = null;
      if (token.value.toLowerCase() == 'binary') {
        extra = this.parseBinaryTypeAnnotion();
      }
      return new MongoBsonValue(
        MongoBsonValueKind.As,
        new MongoBsonAs(value, token, extra)
      );
    }
  }
  parseBinaryTypeAnnotion(): MongoBsonBinaryAllSubType | null {
    let token = this.peek();
    let valueLow = token.value.toLowerCase();
    let t = MongoBsonBinaryAllSubType.allValues().find((e) => valueLow == e);
    if (t) {
      this.next();
      let extra;
      if (t == MongoBsonBinaryAllSubType.Vector) {
        extra = this.parseBinaryTypeAnnotionVector();
      }
      if (t == MongoBsonBinaryAllSubType.UserDefine) {
        const token = this.peek();
        if (token.type == 'Numeric') {
          extra = [this.peek()];
          this.next();
        } else {
          throw new MongoBsonError(
            `shoud be a number after 'user_define' , but '${token.value}'`,
            token.loc
          );
        }
      }
      let r = new MongoBsonBinaryAllSubType(token, extra);
      return r;
    } else {
      return null;
    }
  }

  parseBinaryTypeAnnotionVector(): Token[] {
    let token = this.peek();
    let toLower = token.value.toLowerCase();
    if (toLower == 'float32' || toLower == 'int8') {
      this.next();
      return [token];
    }
  }
  parseArray(): MongoBsonValue[] {
    let values = [];
    while (this.peek().value != ']') {
      values.push(this.parseValueAs());
      if (this.peek().value == ',') {
        this.next();
      }
    }
    return values;
  }

  parseValue(): MongoBsonValue {
    let token = this.peek();
    if (token.type == 'String') {
      this.next();
      return new MongoBsonValue(MongoBsonValueKind.String, token);
    }
    if (token.type == 'Template') {
      this.next();
      return new MongoBsonValue(MongoBsonValueKind.String, token);
    }
    if (token.type == 'Numeric') {
      this.next();
      return new MongoBsonValue(MongoBsonValueKind.Number, token);
    }
    if (token.type == 'Boolean') {
      this.next();
      return new MongoBsonValue(MongoBsonValueKind.Boolean, token);
    }
    if (token.type == 'Null') {
      this.next();
      return new MongoBsonValue(MongoBsonValueKind.Null, token);
    }
    if (token.type == 'Identifier' && token.value.toLowerCase() == 'nan') {
      // special cae
      // this is special value of number.
      this.next();
      return new MongoBsonValue(MongoBsonValueKind.Number, token);
    }

    if (token.value == '-') {
      let startLoc = token.loc.start;
      this.next();
      let tokenNumber = this.peek();
      if (tokenNumber.type == 'Numeric') {
        tokenNumber.value = '-' + tokenNumber.value;
        tokenNumber.loc.start = startLoc;
        this.next();
        return new MongoBsonValue(MongoBsonValueKind.Number, tokenNumber);
      } else {
        throw new MongoBsonError(
          `shoud be a number after '-' , but '${token.value}'`,
          token.loc
        );
      }
    }
    if (token.value == '[') {
      let startLoc = token.loc;
      this.next();
      let values = this.parseArray();
      let endLoc = this.except(']').loc;
      return new MongoBsonValue(
        MongoBsonValueKind.Array,
        new MongoBsonArray(values, startLoc, endLoc)
      );
    }
    if (token.value == '{') {
      let value = this.parseObject();
      return new MongoBsonValue(MongoBsonValueKind.Object, value);
    }
    if (token.type == 'RegularExpression') {
      this.next();
      return new MongoBsonValue(MongoBsonValueKind.RegExp, token);
    }

    throw new MongoBsonError(
      `not valid value,type:'${token.type}' value:'${token.value}'`,
      token.loc
    );
  }

  except(value: string): Token {
    let token = this.peek();
    if (token.value != value) {
      throw new MongoBsonError(
        `expect '${value}' , but '${token.value}'`,
        token.loc
      );
    }
    this.next();
    return token;
  }
  peek(): Token {
    if (this.index < this.tokens.length) {
      return this.tokens[this.index];
    } else {
      // esprima don't produce a EOF token
      // So I make fake
      let loc = this.tokens[this.tokens.length - 1].loc;

      return {
        type: 'EOF',
        value: 'EOF',
        loc: {
          start: { line: loc.end.line, column: loc.end.column },
          end: { line: loc.end.line, column: loc.end.column },
        },
      };
    }
  }
  next() {
    this.index++;
  }
}

function quoteString(quote: string): string {
  const ok = (d: string): [string, boolean] => {
    let s = d + quote + d;
    return [s, (tokenize(s) as Token[]).length == 1];
  };
  {
    const [s, _ok] = ok(`'`);
    if (_ok) {
      return s;
    }
  }
  {
    const [s, _ok] = ok(`"`);
    if (_ok) {
      return s;
    }
  }
  {
    const [s, _ok] = ok('`');
    if (_ok) {
      return s;
    }
  }
  return '`' + quote.replace('`', '\\`') + '`';
}

function validIdentifier(str): boolean {
  let tokens = tokenize(str) as Token[];
  // TOOD Identifier 写对了吗
  return tokens.length == 1 && tokens[0].type == 'Identifier';
}

function toMongoBson(a: any): MongoBsonValue {
  if (a === null) {
    return new MongoBsonValue(MongoBsonValueKind.Null, null);
  }

  switch (typeof a) {
    case 'string':
      return new MongoBsonValue(MongoBsonValueKind.String, {
        value: quoteString(a as unknown as string),
      });
    case 'boolean':
      return new MongoBsonValue(MongoBsonValueKind.Boolean, {
        value: a ? 'true' : 'false',
      });
    case 'number':
      return new MongoBsonValue(MongoBsonValueKind.Number, {
        value: a.toString(),
      });
    case 'bigint':
      return new MongoBsonValue(MongoBsonValueKind.Long, {
        value: a.toString(),
      });
    case 'undefined':
      return new MongoBsonValue(MongoBsonValueKind.Undefined, undefined);
    case 'object':
      if (a instanceof Int32) {
        return new MongoBsonValue(MongoBsonValueKind.Int32, {
          value: a.toString(),
        });
      }
      // Timestamp 必须在Long前面 可能timestamp继承到Long
      if (a instanceof Timestamp) {
        return new MongoBsonValue(MongoBsonValueKind.Timestamp, {
          value: a.toString(),
        });
      }
      if (a instanceof Long) {
        return new MongoBsonValue(MongoBsonValueKind.Long, {
          value: a.toString(),
        });
      }
      if (a instanceof Code) {
        return new MongoBsonValue(MongoBsonValueKind.Code, {
          value: quoteString(a.code),
        });
      }
      if (a instanceof Date) {
        return new MongoBsonValue(MongoBsonValueKind.Date, {
          value: a.toISOString(),
        });
      }
      if (a instanceof RegExp) {
        return new MongoBsonValue(MongoBsonValueKind.RegExp, {
          value: '/' + a.source + '/' + a.flags,
        });
      }
      if (a instanceof ObjectId) {
        return new MongoBsonValue(MongoBsonValueKind.ObjectID, {
          value: a.toHexString(),
        });
      }
      if (a instanceof Double) {
        return new MongoBsonValue(MongoBsonValueKind.Number, {
          value: a.toString(),
        });
      }
      if (a instanceof Decimal128) {
        return new MongoBsonValue(MongoBsonValueKind.Decimal, {
          value: a.toString(),
        });
      }

      if (a instanceof Binary) {
        try {
          let arr = a.toInt8Array();
          return new MongoBsonValue(MongoBsonValueKind.Int8Array, arr);
        } catch (_e) {

        }
        try {
          let arr = a.toFloat32Array();
          return new MongoBsonValue(MongoBsonValueKind.Float32Array, arr);
        } catch (_e) {

        }
        return new MongoBsonValue(MongoBsonValueKind.Binary, a as Binary);
      }
      if (a instanceof Array) {
        let eles = a.map((e) => {
          return toMongoBson(e);
        });
        return new MongoBsonValue(
          MongoBsonValueKind.Array,
          new MongoBsonArray(eles)
        );
      }
      if (a instanceof Object) {
        let kv = [];
        for (const key in a) {
          let keyStr = '';
          if (validIdentifier(key)) {
            keyStr = key;
          } else {
            keyStr = '"' + key + '"';
          }
          kv.push(new MongoBsonKV({ value: keyStr }, toMongoBson(a[key])));
        }
        return new MongoBsonValue(
          MongoBsonValueKind.Object,
          new MongoBsonObject(kv)
        );
      }

      throw new MongoBsonError(`can't convert value ${a.toString()}`);
  }
}

function bsonValue2MongoBson(x: Uint8Array | Document): MongoBsonObject {
  let eles: Document;
  if (x instanceof Uint8Array) {
    eles = BSON.deserialize(x, {
      bsonRegExp: false,
      promoteLongs: false,
      promoteBuffers: false,
      promoteValues: false,
    });
  } else {
    eles = x;
  }

  let defs = [];
  for (const key in eles) {
    let keyStr = '';
    if (validIdentifier(key)) {
      keyStr = key;
    } else {
      keyStr = '"' + key + '"';
    }
    defs.push(new MongoBsonKV({ value: keyStr }, toMongoBson(eles[key])));
  }
  return new MongoBsonObject(defs);
}

interface Position {
  line: number;
  column: number;
}

interface Location {
  start: Position;
  end: Position;
}

export {
  parseMongoBson,
  formatMongoBsonStr,
  bsonValue2MongoBson,
  MongoBsonBinaryAllSubType,
  MongoBsonError,
  MongoBsonObject,
};
