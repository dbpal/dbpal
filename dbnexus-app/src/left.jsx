import { Tree, Alert } from "@arco-design/web-react";
import { useEffect, useState } from "react";
const TreeNode = Tree.Node;
import { invoke } from "@tauri-apps/api/tauri";

function Views({ config, setTabList, setActiveTab }) {
  useEffect(() => {}, [config]);
  let [tableList, setTableList] = useState([]);
  return <TreeNode> </TreeNode>;
}

/// show list of tables
function Tables({ config, setTabList, setActiveTab }) {
  config.log("$$$$$$$$$$$$$$$$", 123);

  let [errMsg, setErrMsg] = useState("");
  let [tableList, setTableList] = useState([]);
  useEffect(() => {
    invoke("list_table", { uuid: config.uuid })
      .catch((e) => {
        setErrMsg(e);
      })
      .then((e) => {
        setTableList(e);
      });
  }, [config]);

  return (
    <>
      {/* <Alert
        closable
        style={{ marginBottom: 20 }}
        type="error"
        title="Error"
        content={errMsg}
      ></Alert> */}

      {tableList.map((e) => {
        console.log("################", e);
        return <TreeNode key={"table_" + e.name} title={e.name}></TreeNode>;
      })}
    </>
  );
}

function OneDBDB({ config, setTabList, setActiveTab }) {
  return (
    <Tree showLine>
      <TreeNode title={config.name} key={config.name}>
        <TreeNode title="Tables" key="tables">
          <Tables
            config={config}
            setActiveTab={setActiveTab}
            setTabList={setTabList}
          />
        </TreeNode>
        <TreeNode title="Views" key="views">
          {/* <Views
            config={config}
            setActiveTab={setActiveTab}
            setTabList={setTabList}
          /> */}
        </TreeNode>
      </TreeNode>
    </Tree>
  );
}

function MultiDBDB({ config, setTabList, setActiveTab }) {}
function OneDataSource({ config, setTabList, setActiveTab }) {
  switch (config["kind"]) {
    case "postgres":
    case "mysql":
    case "sqlserver":
      if (config["db_name"]) {
        return MultiDBDB({ config, setTabList, setActiveTab });
      } else {
        return OneDBDB({ config, setTabList, setActiveTab });
      }
    default:
      return OneDBDB({ config, setTabList, setActiveTab });
  }
}

function LeftTree({ listConfig, setTabList, setActiveTab }) {
  return (
    <div>
      <div>
        {listConfig.map((e) => {
          return (
            <OneDataSource
              key={e.uuid}
              config={e}
              setTabList={setTabList}
              setActiveTab={setActiveTab}
            />
          );
        })}
      </div>
    </div>
  );
}

export { LeftTree };
