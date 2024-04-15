import { useState } from "react";
import reactLogo from "./assets/react.svg";
import { invoke } from "@tauri-apps/api/tauri";
import "./App.css";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
    setGreetMsg(await invoke("greet", { name }));
  }

  // console.log("11111111111");
  // async function xxx() {
  //   let uuid = "xxx123";

  //   await invoke("add_config", {
  //     uuid: uuid,
  //     config: { kind: "sqlite", path: "d:/sqlite.test.db" },
  //   });
  //   await invoke("open_db", { uuid });
  //   let list = await invoke("list_table", { uuid });
  //   console.log(list);
  //   let cols = await invoke("show_columns", { uuid, tableName: "users" });
  //   console.log("cols:", cols);

  //   let table_list = await invoke("show_table", {
  //     uuid,
  //     tableName: "users",
  //     page: 1,
  //     pageSize: 100,
  //   });
  //   console.log("list:", table_list);
  // }

  return (
    <div className="container">
      <h1>Welcome to Tauri!</h1>

      <div className="row">
        <a href="https://vitejs.dev" target="_blank">
          <img src="/vite.svg" className="logo vite" alt="Vite logo" />
        </a>
        <a href="https://tauri.app" target="_blank">
          <img src="/tauri.svg" className="logo tauri" alt="Tauri logo" />
        </a>
        <a href="https://reactjs.org" target="_blank">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>

      <p>Click on the Tauri, Vite, and React logos to learn more.</p>

      <form
        className="row"
        onSubmit={(e) => {
          e.preventDefault();
          greet();
        }}
      >
        <input
          id="greet-input"
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="Enter a name..."
        />
        <button type="submit">Greet</button>
      </form>

      <p>{greetMsg}</p>
    </div>
  );
}

export default App;
