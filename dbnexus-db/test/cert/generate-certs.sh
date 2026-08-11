
#!/bin/bash
# 生成 CA 根证书
openssl req -new -x509 -days 3650 -nodes -text \
  -keyout ca.key \
  -out ca.crt \
  -subj "/CN=TestCA"

# 生成 PostgreSQL 服务端证书（供 Docker 容器使用）
openssl req -new -nodes -text \
  -keyout server.key \
  -out server.req \
  -subj "/CN=localhost"

openssl x509 -req -days 3650 -in server.req \
  -CA ca.crt -CAkey ca.key -CAcreateserial \
  -out server.crt

# 生成客户端证书
openssl req -new -nodes -text \
  -keyout client.key \
  -out client.req \
  -subj "/CN=testuser"

openssl x509 -req -days 3650 -in client.req \
  -CA ca.crt -CAkey ca.key -CAcreateserial \
  -out client.crt

# 清理临时文件
rm server.req client.req ca.srl

# 设置权限（PostgreSQL 需要）
chmod 600 server.key client.key

echo "✅ 证书生成完成！"
echo "📁 文件列表:"
ls -la *.crt *.key