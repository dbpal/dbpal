//@ts-nocheck

const mongoHandle: {
  db: IMongoDB; // db is a database handle can access the database
  BSON: typeof BSONNamespace; // BSON will initialize by 'await import("bson")'
  FS: typeof TauriPluginFsNamespace; // fs will  initialize by await import @tauri-apps/plugin-fs
  HTTP: typeof TauriPluginHttpNamespace; //HTTP will  initialize by await import @tauri-apps/plugin-http
};
