import React, { useState } from "react";
import {
  Form,
  Input,
  Checkbox,
  Button,
  Radio,
  Select,
  Space,
  Typography,
} from "@arco-design/web-react";
const Option = Select.Option;

import { open } from "@tauri-apps/api/dialog";

const FormItem = Form.Item;
const RadioGroup = Radio.Group;

const dbKinds = ["sqlite", "mysql", "postgres", "sqlserver", "redis", "mongo"];

function Sqlite({ db }) {
  const { Title, Paragraph } = Typography;

  const [file, setFile] = useState("");
  function handleOpen() {
    open({
      directory: false,
      multiple: false,
      title: "Select a sqlite db file",
    }).then((e) => {
      setFile(e);
    });
  }

  return (
    <div hidden={!(db === "sqlite")}>
      <Typography>
        <Paragraph>{file}</Paragraph>
        <Paragraph>
          <Button type="primary" onClick={handleOpen}>
            open
          </Button>
        </Paragraph>
        <Paragraph>
          <Button type="primary">Done</Button>
        </Paragraph>
      </Typography>
    </div>
  );
}
function Mysql({ db }) {
  return (
    <div hidden={!(db === "mysql")}>
      <Form autoComplete="off">
        <FormItem label="Username">
          <Input placeholder="host" />
        </FormItem>
        <Button> Done </Button>
      </Form>
    </div>
  );
}

function Postgres({ db }) {
  return <div hidden={!(db === "postgres")}></div>;
}

function Redis({ db }) {
  return <div hidden={!(db === "redis")}></div>;
}
function Mongo({ db }) {
  return <div hidden={!(db === "mongo")}></div>;
}

function Sqlserver({ db }) {
  return <div hidden={!(db === "sqlserver")}></div>;
}

function NewConnection() {
  let [db, setDb] = useState("sqlite");
  function handleChange(value) {
    setDb(value);
  }
  return (
    <div style={{ width: "40%" }}>
      <Select
        placeholder="Select db kind"
        defaultValue="sqlite"
        onChange={handleChange}
        // style={{ width: "235px" }}
      >
        {dbKinds.map((option, index) => (
          <Option key={option} value={option}>
            {option}
          </Option>
        ))}
      </Select>
      <Sqlite db={db} />
      <Mysql db={db} />
      <Postgres db={db} />
      <Redis db={db} />
      <Mongo db={db} />
      <Sqlserver db={db} />
    </div>
  );
}

export default NewConnection;
