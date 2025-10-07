import { BSON } from "bson";

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
    createCollection: (name: string) => IMongoCollection;
    drop: () => Promise<void>;
    listCollectionNames: () => Promise<string[]>;
    runCommand: (doc: BSON.Document) => Promise<BSON.Document>;
}





interface IMongoCollection {
    cuntDocuments: () => Promise<number>;
    createIndex: (index: BSON.Document) => Promise<string>;
    createSearchIndex: (
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



export type { ILogger, IMongoDB, IFindCommon, IFind, IFindOne, IMongoCollection } 