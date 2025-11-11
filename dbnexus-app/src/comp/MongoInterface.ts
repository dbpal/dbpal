import { BSON } from "bson";

interface IMongoDB {
  collection: (name: string) => IMongoCollection;
  createCollection: (name: string) => IMongoCollection;
  drop: () => Promise<void>;
  listCollectionNames: () => Promise<string[]>;
  runCommand: (doc: BSON.Document) => Promise<BSON.Document>;
}

interface IMongoCollection {
  cuntDocuments: () => Promise<number>;
  createIndex: (index: BSON.Document) => Promise<string>;
  createSearchIndex: (index: BSON.Document, name: string | null) => Promise<string>;
  deleteMany: (filter: BSON.Document) => Promise<BSON.Document>;
  deleteOne: (filter: BSON.Document) => Promise<BSON.Document>;
  distinct: (field: string, filter: BSON.Document) => Promise<any[]>;
  drop: () => Promise<void>;
  dropIndex: (name: string) => Promise<void>;
  dropIndexes: () => Promise<void>;
  dropSearchIndex: (name: string) => Promise<void>;
  find: (filter: BSON.Document) => IFindBuilder;
  findOne: (filter: BSON.Document) => IFindOneBuilder;
  findOneAndDelete: (filter: BSON.Document) => Promise<BSON.Document | null>;
  findAndReplace: (
    filter: BSON.Document,
    replacement: BSON.Document,
  ) => Promise<BSON.Document | null>;
  insertMany: (docs: BSON.Document[] | BSON.Document) => Promise<any>;
  insertOne: (doc: BSON.Document) => Promise<any>;
  listIndexNames: () => Promise<string[]>;
  listIndexes: () => Promise<any[]>;
  listSearchIndexes: () => Promise<BSON.Document[]>;
  // collection name;
  name(): string;
  replaceOne: (query: BSON.Document, replacement: BSON.Document) => Promise<any>;
  updateMany: (query: BSON.Document, doc: BSON.Document) => Promise<any>;
  updateOne: (query: BSON.Document, docs: BSON.Document) => Promise<any>;
}

interface IFindCommon {
  max: (doc: BSON.Document) => IFindCommon;
  min: (doc: BSON.Document) => IFindCommon;
  projection: (doc: BSON.Document) => IFindCommon;
  sort: (doc: BSON.Document) => IFindCommon;
  skip: (x: number) => IFindCommon;
  hint: (x: Document) => IFindCommon;
}

interface IFindOneBuilder extends IFindCommon {
  find: () => Promise<BSON.Document>;
}

interface IFindBuilder extends IFindCommon {
  find: () => Promise<BSON.Document[]>;
  limit(limit: number): IFindBuilder;
}

export type { IMongoDB, IFindCommon, IFindBuilder, IFindOneBuilder, IMongoCollection };
