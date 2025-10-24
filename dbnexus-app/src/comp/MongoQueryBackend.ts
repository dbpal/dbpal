// mongodb query interfaces and implementation.

'use strict';
import { loader } from '@monaco-editor/react';
import { invoke } from '@tauri-apps/api/core';
import { BSON } from 'bson';
import { IFindBuilder, IFindOneBuilder, IMongoCollection, IMongoDB } from './MongoInterface';
import interfaceText from "./MongoInterface.ts?raw";


function removeFirstAndLastLine(text: string) {
  const lines = text.split('\n'); // 按换行分割
  if (lines.length <= 2) return ''; // 如果只有一两行，返回空
  return lines.slice(1, -1).join('\n'); // 去掉首尾行
}



class MongoDBQuery implements IMongoDB {
  uuid: string;
  constructor(uuid: string) {
    this.uuid = uuid
  }
  collection(name: string) {
    if (!name) {
      throw `collection name can't be empty`;
    }
    if (typeof name !== "string") {
      throw `name must be string but ${typeof name}`
    }
    return new MongoDBQueryCollection(this.uuid, name);
  }
  createCollection(name: string) {
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

  makeCommonArg(): Record<string, unknown> {
    return {
      name: this.uuid,
      col: this.col,
      query: BSON.serialize(this.filter),
      projection: BSON.serialize(this._projection),
      sort: BSON.serialize(this._sort),
      min: BSON.serialize(this._min),
      max: BSON.serialize(this._max),
      skip: this._skip,
      hint: BSON.serialize(this._hint),
    }
  }
}


class MongoDBQueryFind extends MongoDBQueryFindCommon implements IFindBuilder {
  limit: number;
  async find() {
    const x = this.makeCommonArg();
    x.limit = this.limit;
    return invoke<number[][]>('mongo_query_collection_find', x).then(e => {
      const x = e.map(x => new Uint8Array(x)).map((e) => BSON.deserialize(e));
      return x
    })
  }

  constructor(uuid: string,
    col: string, filter: BSON.Document, limit: number) {
    if (typeof limit != "number") { throw new Error('limit must be number'); }
    if (limit <= 0) { throw new Error('limit must be positive') };
    super(uuid, col, filter);
    this.limit = limit;
  }

}


class MongoDBQueryFindOne extends MongoDBQueryFindCommon implements IFindOneBuilder {
  find() {
    const x = this.makeCommonArg();
    return invoke<number[]>('mongo_query_collection_find_one', x).then(e => {
      const x = BSON.deserialize(new Uint8Array(e));
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
  createSearchIndex(index: BSON.Document, name: string | null) {
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
  find(filter: BSON.Document, limit: number) {
    return new MongoDBQueryFind(this.uuid, this.col, filter, limit)
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
    const extraLib = removeFirstAndLastLine(interfaceText) + `
    const db : IMongoDB;
    const cleanFns : (()=> void)[];
    const bson : any;
        ` ;
    monaco.languages.typescript.javascriptDefaults.addExtraLib(extraLib);
    monaco.languages.typescript.typescriptDefaults.addExtraLib(extraLib);
  }).catch((e) => {
    console.log('init monaco language error:', e)
  });
}

export { initMongoQueryGlobals, MongoDBQuery };