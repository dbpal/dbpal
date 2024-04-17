import { Tabs, Typography } from "@arco-design/web-react";

const TabPane = Tabs.TabPane;

function ContentTabs({ tabList, setTabList, activeTab, setActiveTab }) {
  const handleDeleteTab = (key) => {
    const index = tabList.findIndex((x) => x.key === key);
    const newTabs = tabList.slice(0, index).concat(tabList.slice(index + 1));
    if (key === activeTab && index > -1 && newTabs.length) {
      setActiveTab(
        newTabs[index] ? newTabs[index].key : newTabs[index - 1].key
      );
    }
    if (index > -1) {
      setTabList(newTabs);
    }
  };

  return (
    <Tabs
      editable
      type="card-gutter"
      activeTab={activeTab}
      onDeleteTab={handleDeleteTab}
      onChange={setActiveTab}
      showAddButton={false}
    >
      {tabList.map((x, _) => (
        <TabPane destroyOnHide key={x.key} title={x.title} closable={true}>
          {x.content}
        </TabPane>
      ))}
    </Tabs>
  );
}

export default ContentTabs;
