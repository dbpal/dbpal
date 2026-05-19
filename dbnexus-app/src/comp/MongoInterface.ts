import { BSON } from "bson";

/** Represents a MongoDB database interface */
interface IMongoDB {
  /** Get or create a collection by name */
  collection: (name: string) => IMongoCollection;
  /** Create a new collection with the specified name */
  createCollection: (name: string) => IMongoCollection;
  /** Drop the entire database */
  drop: () => Promise<void>;
  /** List all collection names in the database */
  listCollectionNames: () => Promise<string[]>;
  /** Execute a database command */
  runCommand: (doc: BSON.Document) => Promise<BSON.Document>;
}

interface IMongoCollection {
  /** Count documents in the collection matching the optional filter */
  countDocuments: (filter?: Document) => Promise<number>;
  /** Create an index on the collection with the specified index document */
  createIndex: (index: BSON.Document) => Promise<string>;
  /** Create a search index with the specified index document and optional name */
  createSearchIndex: (index: BSON.Document, name: string | null) => Promise<string>;
  /** Delete all documents matching the filter */
  deleteMany: (filter: BSON.Document) => Promise<BSON.Document>;
  /** Delete a single document matching the filter */
  deleteOne: (filter: BSON.Document) => Promise<BSON.Document>;
  /** Get distinct values of a field across all documents matching the filter */
  distinct: (field: string, filter: BSON.Document) => Promise<any[]>;
  /** Drop the collection */
  drop: () => Promise<void>;
  /** Drop an index by name */
  dropIndex: (name: string) => Promise<void>;
  /** Drop all indexes on the collection */
  dropIndexes: () => Promise<void>;
  /** Drop a search index by name */
  dropSearchIndex: (name: string) => Promise<void>;
  /** Find documents matching the filter and return a builder for chaining queries */
  find: (filter: BSON.Document) => IFindBuilder;
  /** Find a single document matching the filter and return a builder for chaining queries */
  findOne: (filter: BSON.Document) => IFindOneBuilder;
  /** Find and delete a single document matching the filter */
  findOneAndDelete: (filter: BSON.Document) => Promise<BSON.Document | null>;
  /** Find a document matching the filter and replace it */
  findAndReplace: (
    filter: BSON.Document,
    replacement: BSON.Document,
  ) => Promise<BSON.Document | null>;
  /** Insert multiple documents into the collection */
  insertMany: (docs: BSON.Document[] | BSON.Document) => Promise<any>;
  /** Insert a single document into the collection */
  insertOne: (doc: BSON.Document) => Promise<any>;
  /** List all index names in the collection */
  listIndexNames: () => Promise<string[]>;
  /** List all indexes in the collection */
  listIndexes: () => Promise<any[]>;
  /** List all search indexes in the collection */
  listSearchIndexes: () => Promise<BSON.Document[]>;
  /** Get the collection name */
  name(): string;
  /** Replace a single document matching the query with the replacement document */
  replaceOne: (query: BSON.Document, replacement: BSON.Document) => Promise<any>;
  /** Update all documents matching the query */
  updateMany: (query: BSON.Document, doc: BSON.Document) => Promise<any>;
  /** Update a single document matching the query */
  updateOne: (query: BSON.Document, docs: BSON.Document) => Promise<any>;
}

interface IFindCommon {
  /** Apply a maximum value filter for the query */
  max: (doc: BSON.Document) => IFindCommon;
  /** Apply a minimum value filter for the query */
  min: (doc: BSON.Document) => IFindCommon;
  /** Specify the fields to return in query results */
  projection: (doc: BSON.Document) => IFindCommon;
  /** Specify sort order for query results */
  sort: (doc: BSON.Document) => IFindCommon;
  /** Skip the given number of documents in the result set */
  skip: (x: number) => IFindCommon;
  /** Specify an index hint to use for the query */
  hint: (x: Document) => IFindCommon;
}

interface IFindOneBuilder extends IFindCommon {
  /** Execute the query and return a single document */
  find: () => Promise<BSON.Document>;
}

interface IFindBuilder extends IFindCommon {
  /** Execute the query and pass each result document to the handler */
  find: (handle_data: (_: BSON.Document) => void) => Promise<void>;
  /** Execute the query and collect all matching documents */
  findCollect: () => Promise<BSON.Document[]>;

  /** Limit the number of documents returned by the query */
  limit(limit: number): IFindBuilder;
}

export type { IMongoDB, IFindCommon, IFindBuilder, IFindOneBuilder, IMongoCollection };
