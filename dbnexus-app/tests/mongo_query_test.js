// @ts-nocheck
const { db, FS, BSON, HTTP } = mongoHandle;

async function testDB() {
    console.debug('test  db start');
    const x = await db.listCollectionNames();
    console.info('list names', x);
    const pingResult = await db.runCommand({ ping: 1 });
    console.info('ping result:', pingResult);
}

async function testCol() {
    const collection = db.collection('xxx');
    console.info('create collection ok');
    {
        const _collection = db.collection('yyy');
    }
    const insertRet = await collection.insertOne({
        a: 100,
        y: 10000,
        c: 'ssss',
        intValue: new BSON.BSON.Int32(123),
        longValue: new BSON.BSON.Long(456),
    });
    console.info('insert ret is:', insertRet);
    const count = await collection.cuntDocuments();
    console.info('count document is :', count);
    const createIndexRet = await collection.createIndex({ a: 1 });
    console.info('create index ret:', createIndexRet);
    const deleteManyResult = await collection.deleteMany({ c: 2 });
    console.info('delete many ret:', deleteManyResult);
    const deleteOneResult = await collection.deleteOne({ c: 2 });
    console.info('delete one ret:', deleteManyResult);
    const distinctResult = await collection.distinct('a', {});
    console.info('distinct result:', distinctResult);
    const indexNames = await collection.listIndexNames();
    console.info('list Index names:', indexNames);
    const indexes = await collection.listIndexes();
    console.info('list indexes:', indexes);
    const name = collection.name();
    console.info('collection name is:', name);
    const dropIndexRet = await collection.dropIndex('a_1');
    console.info('drop index ret:', dropIndexRet);
    const dropIndexesRet = await collection.dropIndexes();
    console.info('drop indexes ret:', dropIndexesRet);
    const insertManyRet1 = await collection.insertMany({ xxx: '100' });
    console.info('insert many ret1:', insertManyRet1);
    const insertManyRet2 = await collection.insertMany([
        {
            xxx: '100',
            c: 'yyyy',
        },
    ]);
    console.info('insert many ret2:', insertManyRet2);
    const replaceOneRet = await collection.replaceOne({ a: 1 }, { b: 2 });
    console.info('replace one ret:', replaceOneRet);
    try {
        const updateManyRet = await collection.updateMany(
            { a: 1 },
            { $set: { status: 'completed' } },
        );
        console.info('update many ret:', updateManyRet);
    } catch (e) {
        console.info('@@@@@@@@@@@@@@@@@@@', e);
    }
    try {
        const updateOneRet = await collection.updateOne(
            { a: 1 },
            { $set: { status: 'completed' } },
        );
        console.info('update one ret:', updateOneRet);
    } catch (e) {
        console.info('@@@@@@@@@@@@@@@@@@@', e);
    }
    let findManyResult = await collection.find({}).limit(100).find();
    console.info('find many result:', findManyResult);
    let findOneResult = await collection.findOne({}).max({}).find();
    console.info('find one result:', findOneResult);
    const dropResult = await collection.drop();
    console.info('drop result:', dropResult);
}

async function testLimitOffsetPara() {
    let name = 'test_limit_offset';
    let col = db.collection(name);
    await col.deleteMany({});
    await col.createIndex({ a: 1 });
    for (let i = 0; i < 10; i++) {
        await col.insertOne({ a: i });
    }
    // we have 10 record in database
    try {
        let r = await col.find({ a: 1 }).limit(10).projection({ a: 1 }).find();
        console.info('@@@@@@@@@@@@', r);
        console.assert(r[0]['a'] == 1);
        r = await col
            .find({})
            .max({ a: 5 })
            .limit(10)
            .min({ a: 1 })
            .hint({ a: 1 })
            .find();
        console.assert(r.length == 4);
        console.assert(r[0]['a'] == 1);
        r = await col.find({}).limit(3).sort({ a: 1 }).skip(2).find();
        console.assert(r.length == 3, r);
        console.assert(r[0]['a'] == 2);
    } catch (e) {
        console.error('test failed', e);
    }
    try {
        let r = await col.findOne({ a: 1 }).projection({ a: 1 }).find();
        console.info('@@@@@@@@@@@@', r);
        console.assert(r['a'] == 1);
        r = await col
            .findOne({}, 10)
            .max({ a: 5 })
            .min({ a: 1 })
            .hint({ a: 1 })
            .find();
        console.assert(r['a'] == 1);
        r = await col.findOne({}, 3).sort({ a: 1 }).skip(2).find();
        console.assert(r['a'] == 2);
    } catch (e) {
        console.error('test failed', e);
    }
}

async function testFs() {
    try {
        await FS.writeTextFile('/home/yuyang/xxx.json', '123', {
            create: true,
        });
        console.log('write file successfully');
    } catch (e) {
        console.error(e);
    }
}

async function testHttp() {
    try {
        const resp = await HTTP.fetch('https://www.baidu.com/');
        console.log('get http response ok', resp.statusText);
    } catch (e) {
        console.error(e);
    }
}

(async () => {
    await testDB();
    await testCol();
    await testLimitOffsetPara();
    await testFs();
    await testHttp();
})();
