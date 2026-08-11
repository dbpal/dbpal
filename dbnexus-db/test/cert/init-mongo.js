
// 切换到 admin 数据库并创建管理员用户
db = db.getSiblingDB('admin');
db.createUser({
    user: "testuser",
    pwd: "testpass",
    roles: [
        { role: "root", db: "admin" }
    ]
});

// 切换到 testdb 数据库并创建用户
db = db.getSiblingDB('testdb');
db.createUser({
    user: "testuser",
    pwd: "testpass",
    roles: [
        { role: "readWrite", db: "testdb" }
    ]
});
