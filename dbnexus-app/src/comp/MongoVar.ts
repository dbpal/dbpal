// @ts-nocheck
declare const db: IMongoDB; // db is a database handle can access the database
declare const setUpClean: (name: string, f: () => void) => void; // setup a clean function by name and callback
declare const BSON: any; // BSON will initialize by 'await import("bson")'
declare const FS: any; // fs will  initialize by await import @tauri-apps/plugin-fs
