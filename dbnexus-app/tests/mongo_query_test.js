
async function testDB() {
    logger.debug('test  db start');
    const x = await db.listCollectionNames();
    logger.info('list names', x);
    const pingResult = await db.runCommand({ ping: 1 });
    logger.info('ping result:', pingResult);
}

testDB();

async function testCol() {
    const collection = db.collection('xxx');
    logger.info('create collection ok');
    {
        //
        const collection = db.collection('yyy');
    }
    const insertRet = await collection.insertOne({
        a: 100,
        y: 10000,
        c: 'ssss',
        intValue: bsonExtraDataType.newInt32(123),
        longValue: bsonExtraDataType.newLong(123),
        idValue: bsonExtraDataType.newObjectID(),
        tsValue: bsonExtraDataType.newTimeStamp(bsonExtraDataType.newLong(1234)),
        codeValue: bsonExtraDataType.newJavaScriptCode('alert(1)'),
        bValue1: bsonExtraDataType.newBinary([1, 2, 3], 10),
        bValue2: bsonExtraDataType.binaryCreateFromHexString('aa'),
        bValue3: bsonExtraDataType.binaryCreateFromBase64('SGVsbG8='),
        bValue4: bsonExtraDataType.binaryFromInt8Array(new Int8Array([1, 2, 3, 4])),
        bValue5: bsonExtraDataType.binaryFromFloat32Array(new Float32Array([1, 2, 3, 4]))
    });
    logger.info('insert ret is:', insertRet);
    const count = await collection.cuntDocuments();
    logger.info('count document is :', count);
    const createIndexRet = await collection.createIndex({ a: 1 });
    logger.info("create index ret:", createIndexRet);
    const deleteManyResult = await collection.deleteMany({ c: 2 });
    logger.info('delete many ret:', deleteManyResult);
    const deleteOneResult = await collection.deleteOne({ c: 2 });
    logger.info('delete one ret:', deleteManyResult);
    const distinctResult = await collection.distinct('a', {});
    logger.info('distinct result:', distinctResult);
    const indexNames = await collection.listIndexNames();
    logger.info('list Index names:', indexNames);
    const indexes = await collection.listIndexes();
    logger.info('list indexes:', indexes);
    const name = collection.name();
    logger.info("collection name is:", name);
    const dropIndexRet = await collection.dropIndex('a_1');
    logger.info('drop index ret:', dropIndexRet);
    const dropIndexesRet = await collection.dropIndexes();
    logger.info('drop indexes ret:', dropIndexesRet);
    const insertManyRet1 = await collection.insertMany({ xxx: '100' });
    logger.info('insert many ret1:', insertManyRet1);
    const insertManyRet2 = await collection.insertMany([{
        xxx: '100', c: 'yyyy'
    }]);
    logger.info('insert many ret2:', insertManyRet2);
    const replaceOneRet = await collection.replaceOne({ a: 1 }, { b: 2 });
    logger.info('replace one ret:', replaceOneRet);
    try {
        const updateManyRet = await collection.updateMany({ a: 1 }, { $set: { status: "completed" } });
        logger.info('update many ret:', updateManyRet);
    } catch (e) {
        console.log("@@@@@@@@@@@@@@@@@@@", e);
    }
    try {
        const updateOneRet = await collection.updateOne({ a: 1 }, { $set: { status: "completed" } });
        logger.info('update one ret:', updateOneRet);
    } catch (e) {
        console.log("@@@@@@@@@@@@@@@@@@@", e);
    }
    let findManyResult = await collection.find({}).limit(100).find();
    logger.info("find many result:", findManyResult);
    let findOneResult = await collection.findOne({}).max({}).find();
    logger.info("find one result:", findOneResult);
    const dropResult = await collection.drop();
    logger.info('drop result:', dropResult);
}

testCol();


async function testLimitOffsetPara() {
    let name = "test_limit_offset";
    let col = db.collection(name);
    await col.find({ filter: 1 }).max({ max: 1 })
        .min({ min: 1 }).projection({ projection: 3 })
        .limit(10).skip(1).sort({ sort: 1 }).find();

    await col.findOne({ filter: 1 }).max({ max: 1 })
        .min({ min: 1 }).projection({ projection: 3 })
        .skip(10).sort({ sort: 1 }).find();

    await col.drop();
}

testLimitOffsetPara();