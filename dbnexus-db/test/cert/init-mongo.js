
// Switch to the admin database and create an administrator user.
db = db.getSiblingDB('admin');
db.createUser({
    user: "testuser",
    pwd: "testpass",
    roles: [
        { role: "root", db: "admin" }
    ]
});

// Switch to the testdb database and create a user.
db = db.getSiblingDB('testdb');
db.createUser({
    user: "testuser",
    pwd: "testpass",
    roles: [
        { role: "readWrite", db: "testdb" }
    ]
});
