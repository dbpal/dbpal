// mongodb query interfaces and implementation.

import { BSON } from "bson";
import {
  IFindBuilder,
  IFindOneBuilder,
  IMongoCollection,
  IMongoDB,
} from "./MongoInterface";
import interfaceRawText from "./MongoInterface.ts?raw";

import globalVar from "./MongoVar.ts?raw";

import * as monaco from "monaco-editor";
import { commands } from "./bindings";
import bsonTypeStr from "../../node_modules/bson/bson.d.ts?raw";
import fsTypeStr from "../../node_modules/@tauri-apps/plugin-fs/dist-js/index.d.ts?raw";
import httpTypeStr from "../../node_modules/@tauri-apps/plugin-http/dist-js/index.d.ts?raw";

class MongoDBQuery implements IMongoDB {
  uuid: string;
  constructor(uuid: string) {
    this.uuid = uuid;
  }
  collection(name: string) {
    if (!name) {
      throw `collection name can't be empty`;
    }
    if (typeof name !== "string") {
      throw `name must be string but ${typeof name}`;
    }
    return new MongoDBQueryCollection(this.uuid, name);
  }
  createCollection(name: string) {
    return new MongoDBQueryCollection(this.uuid, name);
  }
  drop() {
    return commands.mongoQueryDbDrop(this.uuid);
  }
  listCollectionNames() {
    return commands.mongoQueryDbListCollectionNames(this.uuid);
  }
  runCommand(doc: BSON.Document) {
    return commands.mongoQueryDbRunCommand(this.uuid, BSON.serialize(doc)).then((e) => {
      return BSON.deserialize(new Uint8Array(e));
    });
  }
}

class MongoDBQueryFindCommon {
  uuid: string;
  col: string;
  filter: BSON.Document;
  constructor(uuid: string, col: string, filter: BSON.Document) {
    this.uuid = uuid;
    this.col = col;
    this.filter = filter;
  }
  _max: BSON.Document | null = null;
  _min: BSON.Document | null = null;
  _sort: BSON.Document | null = null;
  _projection: BSON.Document | null = null;
  _skip: number | null = null;
  _hint: BSON.Document | null = null;
  skip(x: number) {
    this._skip = x;
    // this as any make pass the type system because `MongoDBQueryFindCommon`
    // used by multi sub class and `skip` need return subtype not the super type
    return this as any;
  }
  max(doc: BSON.Document) {
    this._max = doc;
    return this;
  }
  min(doc: BSON.Document) {
    this._min = doc;
    return this;
  }
  projection(doc: BSON.Document) {
    this._projection = doc;
    return this;
  }
  sort(doc: BSON.Document) {
    this._sort = doc;
    return this;
  }

  hint(x: Document) {
    this._hint = x;
    return this;
  }

  makeCommonArg() {
    return {
      filter: BSON.serialize(this.filter),
      projection: BSON.serialize(this._projection),
      sort: BSON.serialize(this._sort),
      min: BSON.serialize(this._min),
      max: BSON.serialize(this._max),
      skip: this._skip,
      hint: BSON.serialize(this._hint),
    };
  }
}

class MongoDBQueryFind extends MongoDBQueryFindCommon implements IFindBuilder {
  _limit: number;
  async find() {
    if (!this._limit) {
      throw `limit can't be null`;
    }
    const para = this.makeCommonArg();
    return commands
      .mongoQueryCollectionFind(this.uuid, this.col, { ...para, limit: this._limit })
      .then((e) => {
        const x = e.map((x) => new Uint8Array(x)).map((e) => BSON.deserialize(e));
        return x;
      });
  }

  limit(limit: number): IFindBuilder {
    this._limit = limit;
    return this;
  }

  constructor(uuid: string, col: string, filter: BSON.Document) {
    super(uuid, col, filter);
  }
}

class MongoDBQueryFindOne extends MongoDBQueryFindCommon implements IFindOneBuilder {
  find() {
    const para = this.makeCommonArg();
    return commands
      .mongoQueryCollectionFindOne(this.uuid, this.col, { ...para, limit: null })
      .then((e) => {
        const x = BSON.deserialize(new Uint8Array(e));
        return x;
      });
  }
}
class MongoDBQueryCollection implements IMongoCollection {
  uuid: string;
  col: string;
  constructor(uuid: string, col: string) {
    this.uuid = uuid;
    this.col = col;
  }
  cuntDocuments() {
    return commands.mongoQueryCollectionCountDocs(this.uuid, this.col);
  }
  createIndex(index: BSON.Document) {
    return commands.mongoQueryCollectionCreateIndex(
      this.uuid,
      this.col,
      BSON.serialize(index),
    );
  }
  createSearchIndex(index: BSON.Document, name: string | null) {
    return commands.mongoQueryCollectionCreateSearchIndex(
      this.uuid,
      this.col,
      BSON.serialize(index),
      name,
    );
  }
  deleteMany(filter: BSON.Document) {
    return commands
      .mongoQueryCollectionDeleteMany(this.uuid, this.col, BSON.serialize(filter))
      .then((e) => JSON.parse(e));
  }
  deleteOne(filter: BSON.Document) {
    return commands
      .mongoQueryCollectionDeleteOne(this.uuid, this.col, BSON.serialize(filter))
      .then((e) => JSON.parse(e));
  }
  distinct(field: string, filter: BSON.Document) {
    return commands
      .mongoQueryCollectionDistinct(this.uuid, this.col, BSON.serialize(filter), field)
      .then((e) => JSON.parse(e));
  }
  drop() {
    return commands.mongoQueryCollectionDrop(this.uuid, this.col);
  }
  dropIndex(index: string) {
    return commands.mongoQueryCollectionDropIndex(this.uuid, this.col, index);
  }
  dropIndexes() {
    return commands.mongoQueryCollectionDropIndexes(this.uuid, this.col);
  }
  dropSearchIndex(index: string) {
    return commands.mongoQueryCollectionDropSearchIndex(this.uuid, this.col, index);
  }
  find(filter: BSON.Document) {
    return new MongoDBQueryFind(this.uuid, this.col, filter);
  }
  findOne(filter: BSON.Document) {
    return new MongoDBQueryFindOne(this.uuid, this.col, filter);
  }
  findOneAndDelete(filter: BSON.Document) {
    return commands
      .mongoQueryCollectionFindOneAndDelete(this.uuid, this.col, BSON.serialize(filter))
      .then((e) => (e ? BSON.deserialize(new Uint8Array(e)) : null));
  }
  findAndReplace(filter: BSON.Document, replacement: BSON.Document) {
    return commands
      .mongoQueryCollectionFindOneAndReplace(
        this.uuid,
        this.col,
        BSON.serialize(filter),
        BSON.serialize(replacement),
      )
      .then((e) => (e ? BSON.serialize(new Uint8Array(e)) : null));
  }
  insertMany(docs: BSON.Document[] | BSON.Document) {
    if (!(docs instanceof Array)) {
      docs = [docs];
    }
    return commands
      .mongoQueryCollectionInsertMany(
        this.uuid,
        this.col,
        docs.map((e) => BSON.serialize(e)),
      )
      .then((e) => JSON.parse(e));
  }
  insertOne(doc: BSON.Document) {
    return commands
      .mongoQueryCollectionInsertOne(this.uuid, this.col, BSON.serialize(doc))
      .then((e) => JSON.parse(e));
  }
  listIndexNames() {
    return commands.mongoQueryCollectionListIndexNames(this.uuid, this.col);
  }
  listIndexes() {
    return commands
      .mongoQueryCollectionListIndexes(this.uuid, this.col)
      .then((e) => JSON.parse(e));
  }
  listSearchIndexes() {
    return commands
      .mongoQueryCollectionListSearchIndexes(this.uuid, this.col)
      .then((e) => e.map((e) => BSON.deserialize(new Uint8Array(e))));
  }
  name() {
    return this.col;
  }
  replaceOne(query: BSON.Document, replacement: BSON.Document) {
    return commands
      .mongoQueryCollectionReplaceOne(
        this.uuid,
        this.col,
        BSON.serialize(query),
        BSON.serialize(replacement),
      )
      .then((e) => JSON.parse(e));
  }
  updateMany(query: BSON.Document, replacement: BSON.Document) {
    return commands
      .mongoQueryCollectionUpdateMany(
        this.uuid,
        this.col,
        BSON.serialize(query),
        BSON.serialize(replacement),
      )
      .then((e) => JSON.parse(e));
  }
  updateOne(query: BSON.Document, replacement: BSON.Document) {
    return commands
      .mongoQueryCollectionUpdateOne(
        this.uuid,
        this.col,
        BSON.serialize(query),
        BSON.serialize(replacement),
      )
      .then((e) => JSON.parse(e));
  }
}

function trimInterfaceText(text: string) {
  const lines = text.split("\n"); // 按换行分割
  if (lines.length <= 2) return ""; // 如果只有一两行，返回空
  return lines.slice(1, -2).join("\n"); // 去掉首尾行
}

const interfaceText = trimInterfaceText(interfaceRawText) + globalVar;

function initMongoQueryGlobals() {
  monaco.typescript.typescriptDefaults.addExtraLib(
    `declare namespace BSONNamespace {
    ${bsonTypeStr}
  }`,
    "ts:bson.d.ts",
  );
  monaco.typescript.typescriptDefaults.addExtraLib(
    `declare namespace TauriPluginFsNamespace {
    ${fsTypeStr}
  }`,
    "ts:fs.d.ts",
  );

  monaco.typescript.typescriptDefaults.addExtraLib(
    `declare namespace TauriPluginHttpNamespace {
    ${httpTypeStr}
  }`,
    "ts:http.d.ts",
  );

  monaco.typescript.typescriptDefaults.addExtraLib(interfaceText, "ts:global.d.ts");
}

export { initMongoQueryGlobals, MongoDBQuery, globalVar };
