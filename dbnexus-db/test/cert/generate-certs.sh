#!/bin/bash
DAYS=825

# 1. CA证书
openssl req -new -x509 -days $DAYS -nodes -keyout ca.key -out ca.crt -subj "/CN=TestCA"

# 2. 服务器证书（带 serverAuth EKU）
openssl req -new -nodes -keyout server.key -out server.csr -subj "/CN=localhost"
openssl x509 -req -days $DAYS -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
    -out server.crt \
    -extensions v3_req \
    -extfile <(echo -e "[v3_req]\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth")

# 3. 客户端证书（带 clientAuth EKU）
openssl req -new -nodes -keyout client.key -out client.csr -subj "/CN=testuser"
openssl x509 -req -days $DAYS -in client.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
    -out client.crt \
    -extensions v3_req \
    -extfile <(echo -e "[v3_req]\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=clientAuth")

# 清理 + 权限
rm -f *.csr ca.srl && chmod 600 server.key client.key

echo "✅ 证书生成完成！（带正确的 EKU）"
ls -la *.crt *.key
