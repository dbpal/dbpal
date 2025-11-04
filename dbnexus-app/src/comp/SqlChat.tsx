import { ChatOpenAI } from "@langchain/openai";

const llm = new ChatOpenAI({
  apiKey: "sk-drCWfiTCPZWnAKlKfFxpgwOPCDjWfZnhOJQsYbXe7hCYlUQj", // 写在 .env 里，不要硬编码
  temperature: 0,
  modelName: "moonshot-v1-8k",
  configuration: {
    baseURL: "https://api.moonshot.cn/v1", // 关键：指向 Moonshot
  },
  streaming: false, // 想流式就打开
});
try {
  const res = await llm.invoke("你好啊 什么是大模型");
  console.log(res.content);
} catch (e) {
  console.log("call llvm faile", e);
}

export function hello() {}
