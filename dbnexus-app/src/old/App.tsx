import { Layout } from "@arco-design/web-react";
import MenuBar from "./header.jsx";
import ContentTabs from "./tabs.jsx";
import { invoke } from "@tauri-apps/api/tauri";
import { useEffect, useState } from "react";
import { TabItem } from "./context.tsx";
import { showSysErrMsg } from "./functions.jsx";
import { LeftTree } from "./left.jsx";

const Sider = Layout.Sider;
const Header = Layout.Header;
const Content = Layout.Content;

function App() {
  const [tabs, setTabs] = useState([]);
  function handleSetTabs(e: TabItem[]) {
    setTabs(e as any);
  }

  const [activeTab, setActiveTab] = useState("");
  function handleSetActiveTab(e: string) {
    setActiveTab(e);
  }

  const [listConfig, setListConfig] = useState([]);

  useEffect(() => {
    invoke("list_all_config", {})
      .then((e) => {
        setListConfig(e);
      })
      .catch((e) => {
        showSysErrMsg(e, {});
      });
  }, []);

  return (
    <div>
      <Layout>
        <Header>
          <MenuBar
            setTabList={handleSetTabs}
            setActiveTab={handleSetActiveTab}
          />
        </Header>
        <Layout>
          <Sider
            resizeDirections={["right"]}
            style={{
              minWidth: 250,
              maxWidth: 500,
              minHeight: "800px",
              height: "100%",
            }}
          >
            <LeftTree
              listConfig={listConfig}
              setTabList={handleSetTabs}
              setActiveTab={handleSetActiveTab}
            />
          </Sider>

          <Content>
            <ContentTabs
              tabList={tabs}
              setTabList={handleSetTabs}
              activeTab={activeTab}
              setActiveTab={handleSetActiveTab}
            />
          </Content>
        </Layout>
      </Layout>
    </div>
  );
}

export default App;
