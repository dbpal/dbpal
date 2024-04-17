import React from "react";
import { Form, Input, Checkbox, Button, Radio } from "@arco-design/web-react";

const FormItem = Form.Item;
const RadioGroup = Radio.Group;

function NewConnection() {
  return (
    <Form>
      <FormItem label="Layout">
        <RadioGroup type="button" name="layout">
          <Radio value="horizontal">sqlite</Radio>
          <Radio value="vertical">mysql </Radio>
          <Radio value="inline">postgres</Radio>
        </RadioGroup>
      </FormItem>
      <FormItem
        label="Username"
        field="username"
        tooltip={<div>Username is required </div>}
        rules={[{ required: true }]}
      >
        <Input style={{ width: 270 }} placeholder="please enter your name" />
      </FormItem>
      <FormItem label="Post">
        <Input style={{ width: 270 }} placeholder="please enter your post" />
      </FormItem>
      <FormItem>
        <Checkbox>I have read the manual</Checkbox>
      </FormItem>

      <FormItem>
        <Button type="primary" htmlType="submit">
          Submit
        </Button>
      </FormItem>
    </Form>
  );
}

export default NewConnection;
