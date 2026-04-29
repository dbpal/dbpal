//@ts-nocheck

const mongoHandle: {
  db: IMongoDB; // db is a database handle can access the database
  setUpClean: (name: string, f: () => void) => void; // setup a clean function by name and callback
  BSON: typeof BSONNamespace; // BSON will initialize by 'await import("bson")'
  FS: typeof TauriPluginFsNamespace; // fs will  initialize by await import @tauri-apps/plugin-fs
  HTTP: typeof TauriPluginHttpNamespace; //HTTP will  initialize by await import @tauri-apps/plugin-http
};
