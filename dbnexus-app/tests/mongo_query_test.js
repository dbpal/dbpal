
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
    const isnertRet = await collection.insertOne({ a: 100, y: 10000, c: 'ssss' });
    logger.info('insert ret is:', isnertRet);
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
    const dropResult = await collection.drop();
    logger.info('drop result:', dropResult);
}

testCol();