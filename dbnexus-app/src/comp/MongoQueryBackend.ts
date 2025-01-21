// mongodb query interfaces and implementation.

import { loader } from '@monaco-editor/react';
import { invoke } from '@tauri-apps/api/core';
import { BSON } from 'bson';

//@ts-ignore
interface ILogger {
  trace: (...msg: any[]) => void;
  debug: (...msg: any[]) => void;
  log: (...msg: any[]) => void;
  info: (...msg: any[]) => void;
  warn: (...msg: any[]) => void;
  error: (...msg: any[]) => void;
}

interface IMongoDB {
  collection: (name: string) => IMongoCollection;
  createcollection: (name: string) => IMongoCollection;
  drop: () => Promise<void>;
  listCollectionNames: () => Promise<string[]>;
  runCommand: (doc: BSON.Document) => Promise<BSON.Document>;
}

interface IMongoCollection {
  cuntDocuments: () => Promise<number>;
  createIndex: (index: BSON.Document) => Promise<string>;
  createSerachIndex: (
    index: BSON.Document,
    name: string | null
  ) => Promise<string>;
  deleteMany: (filter: BSON.Document) => Promise<BSON.Document>;
  deleteOne: (filter: BSON.Document) => Promise<BSON.Document>;
  distinct: (field: string, filter: BSON.Document) => Promise<any[]>;
  drop: () => Promise<void>;
  dropIndex: (name: string) => Promise<void>;
  dropIndexes: () => Promise<void>;
  dropSearchIndex: (name: string) => Promise<void>;
  find: (filter: BSON.Document) => IFind;
  findOne: (filter: BSON.Document) => IFindOne;
  findOneAndDelete: (filter: BSON.Document) => Promise<BSON.Document | null>;
  findAndReplace: (
    filter: BSON.Document,
    replacement: BSON.Document
  ) => Promise<BSON.Document | null>;
  insertMany: (docs: BSON.Document[] | BSON.Document) => Promise<any>;
  insertOne: (doc: BSON.Document) => Promise<any>;
  listIndexNames: () => Promise<string[]>;
  listIndexes: () => Promise<any[]>;
  listSearchIndexes: () => Promise<BSON.Document[]>;
  // collection name;
  name(): string;
  replaceOne: (
    query: BSON.Document,
    replacement: BSON.Document
  ) => Promise<any>;
  updateMany: (
    query: BSON.Document,
    doc: BSON.Document
  ) => Promise<any>;
  updateOne: (
    query: BSON.Document,
    docs: BSON.Document
  ) => Promise<any>;
}

interface IFindCommon {
  max: (doc: BSON.Document) => IFind;
  min: (doc: BSON.Document) => IFind;
  projection: (doc: BSON.Document) => IFind;
  sort: (doc: BSON.Document) => IFind;
  skip: (x: number) => IFind;
}
interface IFindOne extends IFindCommon {
  find: () => Promise<BSON.Document>;
}

interface IFind extends IFindCommon {
  limit: (x: number) => IFind;
  find: () => Promise<BSON.Document[]>;
}


class MongoDBQuery implements IMongoDB {
  uuid: string;
  constructor(uuid: string) {
    this.uuid = uuid
  }
  collection(name: string) {
    return new MongoDBQueryCollection(this.uuid, name);
  }
  createcollection(name: string) {
    return new MongoDBQueryCollection(this.uuid, name);
  }
  drop() {
    return invoke<void>('mongo_query_db_drop', { name: this.uuid })
  }
  listCollectionNames() {
    return invoke<string[]>('mongo_query_db_list_collection_names', { name: this.uuid })
  }
  runCommand(doc: BSON.Document) {
    return invoke<number[]>('mongo_query_db_run_command', { name: this.uuid, command: BSON.serialize(doc) }).then(e => {
      return BSON.deserialize(new Uint8Array(e))
    })
  }
}


class MongoDBQueryFindCommon {
  uuid: string;
  col: string;
  filter: BSON.Document;
  constructor(uuid: string,
    col: string, filter: BSON.Document) {
    this.uuid = uuid;
    this.col = col;
    this.filter = filter;
  }
  _max: BSON.Document | null = null;
  _min: BSON.Document | null = null;
  _sort: BSON.Document | null = null;
  _projection: BSON.Document | null = null;
  _skip: number | undefined = undefined;
  skip(x: number) {
    this._skip = x;
    // this as any make pass the type system because `MongoDBQueryFindCommon`
    // used by multi sub class and `skip` need return subtype not the super type
    return this as any;
  }
  max(doc: BSON.Document) {
    this._max = doc;
    return this as any;
  }
  min(doc: BSON.Document) {
    this._min = doc;
    return this as any;
  }
  projection(doc: BSON.Document) {
    this._projection = doc;
    return this as any;
  }
  sort(doc: BSON.Document) {
    this._sort = doc;
    return this as any;
  }
  makeCommonArg(): Record<string, unknown> {
    return {
      name: this.uuid, col: this.col, query: BSON.serialize(this.filter),
    }
  }
}

class MongoDBQueryFind extends MongoDBQueryFindCommon implements IFind {
  _limit: number | undefined = undefined;
  limit(x: number) {
    this._limit = x;
    return this;
  }
  find() {
    let x = this.makeCommonArg();
    x.limit = this._limit;
    return invoke<number[][]>('mongo_query_collection_find', x).then(e => {
      let x = e.map(x => new Uint8Array(x)).map((e) => BSON.deserialize(e));
      return x
    })
  }
}

class MongoDBQueryFindOne extends MongoDBQueryFindCommon implements IFindOne {
  find() {
    let x = this.makeCommonArg();
    return invoke<number[]>('mongo_query_collection_find_one', x).then(e => {
      let x = BSON.deserialize(new Uint8Array(e));
      return x
    })
  }
}
class MongoDBQueryCollection implements IMongoCollection {
  uuid: string;
  col: string
  constructor(uuid: string,
    col: string) {
    this.uuid = uuid;
    this.col = col;
  }
  cuntDocuments() {
    return invoke<number>('mongo_query_collection_count_docs', { name: this.uuid, col: this.col })
  }
  createIndex(index: BSON.Document) {
    return invoke<string>('mongo_query_collection_create_index', { name: this.uuid, col: this.col, index: BSON.serialize(index) })
  }
  createSerachIndex(index: BSON.Document, name: string | null) {
    return invoke<string>('mongo_query_collection_create_search_index', {
      name: this.uuid, col: this.col, index: BSON.serialize(index),
      indexName: name
    })
  }
  deleteMany(filter: BSON.Document) {
    return invoke<string>('mongo_query_collection_delete_many', { name: this.uuid, col: this.col, query: BSON.serialize(filter) })
      .then(e => JSON.parse(e));
  }
  deleteOne(filter: BSON.Document) {
    return invoke<string>('mongo_query_collection_delete_one', { name: this.uuid, col: this.col, query: BSON.serialize(filter) })
      .then(e => JSON.parse(e))
  }
  distinct(field: string, filter: BSON.Document) {
    return invoke<string>('mongo_query_collection_distinct', { name: this.uuid, col: this.col, query: BSON.serialize(filter), field })
      .then(e => JSON.parse(e))
  }
  drop() {
    return invoke<void>('mongo_query_collection_drop', { name: this.uuid, col: this.col })
  }
  dropIndex(index: string) {
    return invoke<void>('mongo_query_collection_drop_index', { name: this.uuid, col: this.col, index })
  }
  dropIndexes() {
    return invoke<void>('mongo_query_collection_drop_indexes', { name: this.uuid, col: this.col })
  }
  dropSearchIndex(index: string) {
    return invoke<void>('mongo_query_collection_drop_search_index', { name: this.uuid, col: this.col, index })
  }
  find(filter: BSON.Document) {
    return new MongoDBQueryFind(this.uuid, this.col, filter)
  }
  findOne(filter: BSON.Document) {
    return new MongoDBQueryFindOne(this.uuid, this.col, filter)
  }
  findOneAndDelete(filter: BSON.Document) {
    return invoke<number[] | null>('mongo_query_collection_find_one_and_delete', { name: this.uuid, col: this.col, query: BSON.serialize(filter) })
      .then(e => e ? BSON.deserialize(new Uint8Array(e)) : null)
  }
  findAndReplace(filter: BSON.Document, replacement: BSON.Document) {

    return invoke<number[] | null>('mongo_query_collection_find_one_and_replace', {
      name: this.uuid, col: this.col, query: BSON.serialize(filter),
      replacement: BSON.serialize(replacement),
    }).then(e => e ? BSON.serialize(new Uint8Array(e)) : null)

  }
  insertMany(docs: BSON.Document[] | BSON.Document) {
    if (!(docs instanceof Array)) {
      docs = [docs]
    }
    return invoke<string>('mongo_query_collection_insert_many', {
      name: this.uuid, col: this.col, data: docs.map(e => BSON.serialize(e))
    }).then(e => JSON.parse(e))
  }
  insertOne(doc: BSON.Document) {
    return invoke<string>('mongo_query_collection_insert_one', {
      name: this.uuid, col: this.col,
      doc: BSON.serialize(doc)
    }).then(e => JSON.parse(e))
  }
  listIndexNames() {
    return invoke<string[]>('mongo_query_collection_list_index_names', { name: this.uuid, col: this.col, });

  }
  listIndexes() {
    return invoke<string>('mongo_query_collection_list_indexes', { name: this.uuid, col: this.col, })
      .then(e => JSON.parse(e))
  }
  listSearchIndexes() {
    return invoke<number[][]>('mongo_query_collection_list_search_indexes', { name: this.uuid, col: this.col, })
      .then(e => e.map(e => BSON.deserialize(new Uint8Array(e))))
  }
  name() {
    return this.col
  }
  replaceOne(query: BSON.Document, replacement: BSON.Document) {
    return invoke<string>('mongo_query_collection_replace_one', {
      name: this.uuid, col: this.col,
      query: BSON.serialize(query), replacement: BSON.serialize(replacement)
    }).then(e => JSON.parse(e))
  }
  updateMany(query: BSON.Document, replacement: BSON.Document) {
    return invoke<string>('mongo_query_collection_update_many', {
      name: this.uuid, col: this.col,
      query: BSON.serialize(query), replacement: BSON.serialize(replacement)
    }).then(e => JSON.parse(e))
  }
  updateOne(query: BSON.Document, replacement: BSON.Document) {
    return invoke<string>('mongo_query_collection_update_one', {
      name: this.uuid, col: this.col,
      query: BSON.serialize(query), replacement: BSON.serialize(replacement)
    }).then(e => JSON.parse(e))
  }
}


function initMongoQueryGlobals() {
  loader.init().then((monaco) => {
    monaco.languages.typescript.javascriptDefaults.addExtraLib(`
  interface ILogger {
    trace: (...msg: any[]) => void;
    debug: (...msg: any[]) => void;
    log: (...msg: any[]) => void;
    info: (...msg: any[]) => void;
    warn: (...msg: any[]) => void;
    error: (...msg: any[]) => void;
  }

interface IMongoDB {
  collection: (name: string) => IMongoCollection;
  createcollection: (name: string) => IMongoCollection;
  drop: () => Promise<void>;
  listCollectionNames: () => Promise<string[]>;
  runCommand: (doc: BSON.Document) => Promise<BSON.Document>;
}

interface IMongoCollection {
  cuntDocuments: () => Promise<number>;
  createIndex: (index: BSON.Document) => Promise<string>;
  createSerachIndex: (
    index: BSON.Document,
    name: string | null
  ) => Promise<string>;
  deleteMany: (filter: BSON.Document) => Promise<BSON.Document>;
  deleteOne: (filter: BSON.Document) => Promise<BSON.Document>;
  distinct: (field: string, filter: BSON.Document) => Promise<any[]>;
  drop: () => Promise<void>;
  dropIndex: (name: string) => Promise<void>;
  dropIndexes: () => Promise<void>;
  dropSearchIndex: (name: string) => Promise<void>;
  find: (filter: BSON.Document) => IFind;
  findOne: (filter: BSON.Document) => IFindOne;
  findOneAndDelete: (filter: BSON.Document) => Promise<BSON.Document | null>;
  findAndReplace: (
    filter: BSON.Document,
    replacement: BSON.Document
  ) => Promise<BSON.Document | null>;
  insertMany: (docs: BSON.Document[] | BSON.Document) => Promise<any>;
  insertOne: (doc: BSON.Document) => Promise<any>;
  listIndexNames: () => Promise<string[]>;
  listIndexes: () => Promise<any[]>;
  listSearchIndexes: () => Promise<BSON.Document[]>;
  // collection name;
  name(): string;
  replaceOne: (
    query: BSON.Document,
    replacement: BSON.Document
  ) => Promise<any>;
  updateMany: (
    query: BSON.Document,
    doc: BSON.Document
  ) => Promise<any>;
  updateOne: (
    query: BSON.Document,
    docs: BSON.Document
  ) => Promise<any>;
}

interface IFindCommon {
  max: (doc: BSON.Document) => IFind;
  min: (doc: BSON.Document) => IFind;
  projection: (doc: BSON.Document) => IFind;
  sort: (doc: BSON.Document) => IFind;
  skip: (x: number) => IFind;
}
interface IFindOne extends IFindCommon {
  find: () => Promise<BSON.Document>;
}

interface IFind extends IFindCommon {
  limit: (x: number) => IFind;
  find: () => Promise<BSON.Document[]>;
}
  
    var db : IMongoDB;
    var logger : ILogger;
  
  
        `);
  });
}


export { initMongoQueryGlobals, MongoDBQuery };
