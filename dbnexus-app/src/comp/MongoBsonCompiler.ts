import { tokenize } from 'esprima';
import { BSON, EJSON, ObjectId } from 'bson';

class MongoBsonObject {
  constructor(
    public startLoc: Location,
    public eles: MongoBsonKV[],
    public endLoc: Location
  ) {}
}

class MongoBsonKV {
  constructor(public key: Token, public value: MongoBsonValue) {}
}

class MongoBsonAs {
  constructor(public value: MongoBsonValue, public kind: Token) {}
}

enum MongoBsonValueKind {
  Number,
  String,
  As,
  Array,
  Object,
  RegExp,
}

class MongoBsonValue {
  constructor(
    public kind: MongoBsonValueKind,
    public value: Token | MongoBsonObject | MongoBsonAs | MongoBsonArray
  ) {}

  format(ident: number, newLine: boolean, tabSize: number): string {
    const doIdent = (newLine: boolean): string => {
      if (newLine) {
        return ' '.repeat(tabSize).repeat(ident);
      } else {
        return '';
      }
    };
    let s = '';
    switch (this.kind) {
      case MongoBsonValueKind.Number:
        return (this.value as Token).value;
      case MongoBsonValueKind.String:
        return (this.value as Token).value;
      case MongoBsonValueKind.As:
        let as = this.value as MongoBsonAs;
        s = as.value.format(ident, false, tabSize);
        s += ` as ${as.kind.value}`;
        return s;
      case MongoBsonValueKind.Array:
        let valueArr = this.value as MongoBsonArray;
        s = '[';
        newLine = newLine || this.elementsSize() > 100;
        if (newLine) {
          s += '\n';
        }
        valueArr.eles.forEach((e, index) => {
          s += doIdent(newLine);
          s += e.format(ident + 1, false, tabSize);
          if (index != valueArr.eles.length - 1) {
            s += ',';
          }
          if (newLine) {
            s += '\n';
          }
        });
        s += ']';
        if (newLine) {
          s += '\n';
        }
        return s;
      case MongoBsonValueKind.Object:
        let valueObj = this.value as MongoBsonObject;
        s = '{';
        newLine = newLine || this.elementsSize() > 100;
        if (newLine) {
          s += '\n';
        }
        valueObj.eles.forEach((e, index) => {
          s += doIdent(newLine);
          s += e.key.value + ': ';
          s += e.value.format(ident + 1, false, tabSize);
          if (index != valueObj.eles.length - 1) {
            s += ',';
          }
          if (newLine) {
            s += '\n';
          }
        });
        s += '}';
        if (newLine) {
          s += '\n';
        }
        return s;
    }
  }

  elementsSize(): number {
    let count = 0;
    switch (this.kind) {
      case MongoBsonValueKind.Number:
      case MongoBsonValueKind.String:
        return (this.value as Token).value.length;
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
    public startLoc: Location,
    public eles: MongoBsonValue[],
    public endLoc: Location
  ) {}
}

function parseMongoBson(input: string): MongoBsonObject | MongoBsonError {
  let token = tokenize(input, { loc: true }) as Token[];
  let parser = new Parser(token);
  try {
    let r = parser.parseObject();
    return r;
  } catch (e) {
    return e;
  }
}

function formatMongoBson(
  input: string,
  tabSize: number
): string | MongoBsonError {
  let x = parseMongoBson(input);
  if (x instanceof MongoBsonError) {
    return x;
  }
  let ast = x as MongoBsonObject;
  let s = new MongoBsonValue(MongoBsonValueKind.Object, ast).format(
    1,
    true,
    tabSize
  );
  return s;
}

interface Token {
  type: string;
  value: string;
  loc: Location;
}

class MongoBsonError {
  constructor(public loc: Location, public msg: string) {}
}

class Parser {
  index: number = 0;
  constructor(public tokens: Token[]) {}
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
    return new MongoBsonObject(startLoc, defs, endLoc);
  }

  parseObjectDefs(): MongoBsonKV {
    let token = this.peek();
    if (token.type != 'Identifier' && token.type != 'String') {
      throw new MongoBsonError(
        token.loc,
        `expect identifer or string,but '${token.value}'`
      );
    }
    this.next();
    let tokenColon = this.peek();
    if (tokenColon.value != ':') {
      throw new MongoBsonError(
        token.loc,
        `expect colon or string,but '${token.value}'`
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
      return new MongoBsonValue(
        MongoBsonValueKind.As,
        new MongoBsonAs(value, token)
      );
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
    if (token.type == 'Numeric') {
      this.next();
      return new MongoBsonValue(MongoBsonValueKind.Number, token);
    }
    if (token.value == '[') {
      let startLoc = token.loc;
      this.next();
      let values = this.parseArray();
      let endLoc = this.except(']').loc;
      return new MongoBsonValue(
        MongoBsonValueKind.Array,
        new MongoBsonArray(startLoc, values, endLoc)
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
      token.loc,
      `not valid value,type:'${token.type}' value:'${token.value}'`
    );
  }

  except(value: string): Token {
    let token = this.peek();
    if (token.value != value) {
      throw new MongoBsonError(
        token.loc,
        `expect '${value}' , but '${token.value}'`
      );
    }
    this.next();
    return token;
  }
  peek(): Token {
    return this.tokens[this.index];
  }
  next() {
    this.index++;
  }
}

function bsonValue2MongoBson(x) {
  let d = BSON.deserialize(x, {});
}

interface Position {
  line: number;
  column: number;
}
interface Location {
  start: Position;
  end: Position;
}

export { parseMongoBson, formatMongoBson };
