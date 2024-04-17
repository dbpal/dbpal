import { Layout } from "@arco-design/web-react";
import MenuBar from "./header.jsx";
import ContentTabs from "./tabs.jsx";

import { useState } from "react";
import { TabItem } from "./context.tsx";
const Sider = Layout.Sider;
const Header = Layout.Header;
const Content = Layout.Content;

function App() {
  let [tabs, setTabs] = useState([]);
  function handleSetTabs(e: TabItem[]) {
    setTabs(e as any);
  }

  let [activeTab, setActiveTab] = useState("");
  function handleSetActiveTab(e: string) {
    setActiveTab(e);
  }
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
              minWidth: 150,
              maxWidth: 500,
              height: 800,
            }}
          >
            Sider
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
