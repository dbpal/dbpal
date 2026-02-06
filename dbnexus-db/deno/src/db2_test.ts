import { DB2Server } from "./db2.ts";

Deno.test("db2 test", async function () {
  let db = await DB2Server.connect({
    username: "db2inst1",
    password: "db2inst1-pwd",
    host: "localhost",
    port: 50000,
    db: "TSKDB",
    driver_name: "Driver_IBM_DB2",
  });
  {
    // check conn

    const d = await db.checkConn();
    console.log("check conn duration", d, "ms");
  }

  {
    // schema

    const schemaes = await db.listSchema();
    console.log("schemaes", schemaes);
  }

  { // list table
    const x = await db.openSchema("TEST");
    const tables = await x.listTable();
    console.log("tables", tables);

    for (const t of tables) {
      const cols = await x.showTableColumns(t.name);
      console.log("cols", cols);
    }
  }
});
