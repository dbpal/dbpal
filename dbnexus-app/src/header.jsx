import { Menu } from "@arco-design/web-react";
import { Dropdown, Button, Space } from "@arco-design/web-react";
import { IconDown } from "@arco-design/web-react/icon";
import { invoke } from "@tauri-apps/api/tauri";
import NewConnection from "./new_connection";

const MenuItem = Menu.Item;
const SubMenu = Menu.SubMenu;

//@ts-ignore
function FileMenu({ setTabList, setActiveTab }) {
  const dropList = (
    <Menu>
      <Menu.Item key="new_connection" onClick={on_new_connection}>
        New Connection
      </Menu.Item>
      <Menu.Item key="exit" onClick={on_exit}>
        Exit
      </Menu.Item>
    </Menu>
  );

  let i = 0;
  function on_new_connection() {
    let name = "new_connection:" + i;
    setTabList((prev) => {
      return [
        ...prev,
        {
          title: "New Connection",
          key: name,
          content: <NewConnection />,
        },
      ];
    });
    i++;
    setActiveTab(name);
  }
  function on_exit() {
    invoke("exit", { code: 0 });
  }

  return (
    <Space size="large" className="dropdown-demo">
      <Dropdown droplist={dropList} trigger="click" position="br">
        <Button type="text">
          File
          <IconDown />
        </Button>
      </Dropdown>
    </Space>
  );
}

function MenuBar({ setTabList, setActiveTab }) {
  return (
    <div>
      <Menu mode="horizontal" defaultSelectedKeys={["1"]}>
        <MenuItem key="files">
          <FileMenu setTabList={setTabList} setActiveTab={setActiveTab} />
        </MenuItem>
        <MenuItem key="login">Login </MenuItem>
      </Menu>
    </div>
  );
}

export default MenuBar;
