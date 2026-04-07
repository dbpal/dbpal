use std::future::Future;
use std::pin::Pin;

use std::sync::{Arc, OnceLock};
use std::task::Poll;
use std::vec;

use crate::db::GetBackendKind;
use crate::language_server::{
    in_pos, CompletionItem, CompletionKind, CompletionList, LanguageServer,
};

use crate::redis::{get_command_from_sql, redis_tokenize, RedisDB};
use futures_core::future::BoxFuture;
use futures_util::{FutureExt, StreamExt};
use redis::aio::MultiplexedConnection;
use redis::{AsyncCommands, AsyncIter};
use tokio::sync::Mutex;

const MAX_COMPLETION_ITEMS: usize = 200;

impl RedisDB {
    pub fn get_query_conn_by_id(
        &self,
        query_id: &String,
    ) -> anyhow::Result<Arc<Mutex<MultiplexedConnection>>> {
        let conn = self.query_connection.get(query_id);
        match conn {
            Some(x) => anyhow::Ok(x.clone()),
            None => {
                anyhow::bail!(format!(
                    "should be able get connection,connection id:{}",
                    self.completion_id.get()
                ));
            }
        }
    }
    fn get_query_conn(&self) -> anyhow::Result<Arc<Mutex<MultiplexedConnection>>> {
        self.get_query_conn_by_id(&self.completion_id.get())
    }
    async fn completion_keys(&self, pattern: &str) -> anyhow::Result<CompletionList> {
        log::trace!("completion on key pattern:{}", pattern);
        let conn = self.get_query_conn()?;
        let mut conn = conn.lock().await;
        let mut stream = conn.scan_match(format!("*{}*", pattern)).await?;
        let mut incomplete = false;
        let mut items = Vec::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            if let Ok(x) = String::from_utf8(item) {
                items.push(CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Table,
                    desc: None,
                });
                if items.len() >= MAX_COMPLETION_ITEMS {
                    incomplete = true;
                    break;
                }
            }
        }
        anyhow::Ok(CompletionList {
            items,
            incomplete,
            from_dot: false,
        })
    }

    async fn completion_hash_keys(
        &self,
        hash_key: &str,
        p: &str,
    ) -> anyhow::Result<CompletionList> {
        let conn = self.get_query_conn()?;
        let mut conn = conn.lock().await;
        let mut stream: AsyncIter<'_, Vec<u8>> =
            conn.hscan_match(hash_key, format!("*{}*", p)).await?;
        let mut ret = vec![];
        let mut incomplete = false;
        while let Some(k) = stream.next().await {
            let k = k?;
            if let Ok(x) = String::from_utf8(k) {
                ret.push(x);
                if ret.len() >= MAX_COMPLETION_ITEMS {
                    incomplete = true;
                    break;
                }
            }
        }
        anyhow::Ok(CompletionList {
            items: ret
                .into_iter()
                .map(|x| CompletionItem {
                    kind: CompletionKind::Column,
                    item: x.into(),
                    desc: None,
                })
                .collect::<Vec<_>>(),
            incomplete,
            from_dot: false,
        })
    }

    async fn completion_set_member(&self, key: &str, p: &str) -> anyhow::Result<CompletionList> {
        let conn = self.get_query_conn()?;
        let mut conn = conn.lock().await;
        let mut stream = conn
            .sscan_match::<_, _, Vec<u8>>(key, format!("*{}*", p))
            .await?;
        let mut members = vec![];
        let mut incomplete = false;
        while let Some(k) = stream.next().await {
            let k = k?;
            if let Ok(x) = String::from_utf8(k) {
                members.push(x);
                if members.len() >= MAX_COMPLETION_ITEMS {
                    incomplete = true;
                    break;
                }
            }
        }
        anyhow::Ok(CompletionList {
            items: members
                .into_iter()
                .map(|x| CompletionItem {
                    kind: CompletionKind::Column,
                    item: x.into(),
                    desc: None,
                })
                .collect(),
            incomplete,
            from_dot: false,
        })
    }

    async fn completion_zset_member(
        &self,
        key: &str,
        member: &str,
    ) -> anyhow::Result<CompletionList> {
        let conn = self.get_query_conn()?;
        let mut conn = conn.lock().await;
        let mut stream = conn
            .zscan_match::<_, _, (Vec<u8>, f64)>(key, format!("*{}*", member))
            .await?;
        let mut members = vec![];
        let mut incomplete = false;
        while let Some(item) = stream.next_item().await {
            let item = item?;
            let (key, score) = item;
            if let Ok(x) = String::from_utf8(key) {
                members.push((score, x));
                if members.len() >= MAX_COMPLETION_ITEMS {
                    incomplete = true;
                    break;
                }
            }
        }
        anyhow::Ok(CompletionList {
            items: members
                .into_iter()
                .map(|(score, key)| CompletionItem {
                    kind: CompletionKind::Column,
                    item: key.into(),
                    desc: Some(score.to_string().into()),
                })
                .collect(),
            incomplete,
            from_dot: false,
        })
    }

    fn make_auto_completion_fn_by_command<'a, 'b>(
        &'a self,
        command: &'b str,
        patterns: &'b [String],
    ) -> anyhow::Result<Vec<BoxFuture<'b, anyhow::Result<CompletionList>>>>
    where
        'a: 'b,
    {
        if is_simple_command(command) {
            return anyhow::Ok(vec![async {
                let x: Vec<_> = get_simple_command_sub(command)
                    .iter()
                    .map(|x| x.to_uppercase())
                    .collect();
                anyhow::Ok(CompletionList {
                    items: x
                        .into_iter()
                        .map(|x| CompletionItem {
                            kind: CompletionKind::Command,
                            item: x.into(),
                            desc: None,
                        })
                        .collect(),
                    incomplete: false,
                    from_dot: false,
                })
            }
            .boxed()]);
        }

        /// dummy auto completion don't return anything
        #[derive(Clone, Copy, Debug)]
        struct Dummy {}
        impl Future for Dummy {
            type Output = anyhow::Result<CompletionList>;

            fn poll(self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
                Poll::Ready(anyhow::Ok(vec![].into()))
            }
        }
        let dummy = Box::pin(Dummy {});

        let fns: Vec<BoxFuture<'_, anyhow::Result<CompletionList>>> = match command
            .to_uppercase()
            .as_str()
        {
            "APPEND" | "SET" => {
                let key = patterns.first();
                match key {
                    Some(key) => {
                        vec![async move { self.completion_keys(key).await }.boxed()]
                    }
                    None => vec![],
                }
            }
            "BGREWRITEAOF" => {
                vec![]
            }
            "BGSAVE" => {
                vec![async move {
                    anyhow::Ok(
                        vec![
                            CompletionItem {
                                item: "SCHEDULE".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "FORCE".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "SCHEDULE FORCE".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "SCHEDULE CRON".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "SCHEDULE INTERVAL".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                        ]
                        .into(),
                    )
                }
                .boxed()]
            }
            "BITCOUNT" => {
                let key = patterns.first();
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }
            "BITFIELD" => {
                let key = patterns.first();
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }

            "BITOP" => {
                let mut vs = vec![async move {
                    let ret = vec![
                        CompletionItem {
                            item: "AND".into(),
                            kind: CompletionKind::Text,
                            desc: None,
                        },
                        CompletionItem {
                            item: "OR".into(),
                            kind: CompletionKind::Text,
                            desc: None,
                        },
                        CompletionItem {
                            item: "XOR".into(),
                            kind: CompletionKind::Text,
                            desc: None,
                        },
                        CompletionItem {
                            item: "NOT".into(),
                            kind: CompletionKind::Text,
                            desc: None,
                        },
                        CompletionItem {
                            item: "DIFF".into(),
                            kind: CompletionKind::Text,
                            desc: None,
                        },
                        CompletionItem {
                            item: "DIFF1".into(),
                            kind: CompletionKind::Text,
                            desc: None,
                        },
                        CompletionItem {
                            item: "ANDOR".into(),
                            kind: CompletionKind::Text,
                            desc: None,
                        },
                        CompletionItem {
                            item: "ONE".into(),
                            kind: CompletionKind::Text,
                            desc: None,
                        },
                    ];
                    anyhow::Ok(CompletionList::from(ret))
                }
                .boxed()];
                let x = patterns
                    .iter()
                    .skip(1)
                    .map(|x| async move { self.completion_keys(x).await }.boxed())
                    .collect::<Vec<_>>();

                vs.extend(x);
                vs
            }
            "BITPOS" => {
                let key = patterns.first();
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }
            "BLPOP" | "BRPOP" => patterns
                .iter()
                .map(|key| async move { self.completion_keys(key).await }.boxed())
                .collect::<Vec<_>>(),

            "BRPOPLPUSH" => patterns
                .iter()
                .enumerate()
                .map(|(index, key)| match index {
                    0 | 1 => async move { self.completion_keys(key).await }.boxed(),
                    _ => dummy.clone(),
                })
                .collect::<Vec<_>>(),
            "DECR" | "DECRBY" => {
                let key = patterns.first();
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }

            "DEL" => patterns
                .iter()
                .map(|x| async move { self.completion_keys(x).await }.boxed())
                .collect(),
            "DUMP" => {
                let key = patterns.first();
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }
            "EXISTS" => {
                let x = patterns
                    .iter()
                    .map(|key| async move { self.completion_keys(key).await }.boxed())
                    .collect::<Vec<_>>();
                x
            }

            "EXPIRE" | "EXPIREAT" => patterns
                .iter()
                .enumerate()
                .map(|(index, key)| {
                    match index {
                        0 => async move { self.completion_keys(key).await }.boxed(),
                        _ => dummy.clone(),
                    }
                    .boxed()
                })
                .collect::<Vec<_>>(),

            "GEOADD" | "GEORADIUS" => {
                // geo data use zset to store.
                if !patterns.is_empty() {
                    vec![async move { self.completion_keys(&patterns[0]).await }.boxed()]
                } else {
                    vec![]
                }
            }
            "GEODIST" | "GEOHASH" | "GEOPOS" | "GEORADIUSBYMEMBER" => patterns
                .iter()
                .enumerate()
                .map(|(index, p)| match index {
                    0 => async move { self.completion_keys(&patterns[0]).await }.boxed(),
                    _ => async move { self.completion_zset_member(&patterns[0], p).await }.boxed(),
                })
                .collect(),
            "GET" | "GETBIT" | "GETRANGE" | "GETSET" => {
                if !patterns.is_empty() {
                    vec![async move { self.completion_keys(&patterns[0]).await }.boxed()]
                } else {
                    vec![]
                }
            }
            "HDEL" | "HEXISTS" | "HGET" => {
                patterns
                    .iter()
                    .enumerate()
                    .map(|(index, p)| {
                        match index {
                            0 => {
                                // suggest hash key
                                async move { self.completion_keys(p).await }.boxed()
                            }
                            _ => {
                                let key = &patterns[0];
                                // hash key store in pattern[0]
                                async move { self.completion_hash_keys(key.as_str(), p).await }
                                    .boxed()
                            }
                        }
                    })
                    .collect()
            }
            "HGETALL" => {
                if !patterns.is_empty() {
                    vec![async move { self.completion_keys(&patterns[0]).await }.boxed()]
                } else {
                    vec![]
                }
            }
            "HINCRBY" | "HINCRBYFLOAT" => {
                patterns
                    .iter()
                    .enumerate()
                    .map(|(index, p)| {
                        match index {
                            0 => {
                                async move { self.completion_keys(p).await }.boxed()

                                // suggest hash key
                            }
                            1 => {
                                let key = &patterns[0];

                                // hash key store in pattern[0]
                                async move { self.completion_hash_keys(key.as_str(), p).await }
                                    .boxed()
                            }
                            _ => dummy.clone(),
                        }
                    })
                    .collect()
            }
            "HKEYS" | "HLEN" => {
                if !patterns.is_empty() {
                    vec![async move { self.completion_keys(&patterns[0]).await }.boxed()]
                } else {
                    vec![]
                }
            }
            "HMGET" => {
                patterns
                    .iter()
                    .enumerate()
                    .map(|(index, pattern)| {
                        match index {
                                0 => {
                                    async move {
                                        self.completion_keys(&patterns[0])
                                            .await
                                    }
                                    .boxed()
                                    // suggest hash key
                                }
                                _ => async move {
                                    self.completion_hash_keys(&patterns[0], pattern).await
                                }
                                .boxed(),
                            }
                    })
                    .collect()
            }
            "HMSET" | "HSET" => patterns
                .iter()
                .enumerate()
                .map(|(index, p)| match index {
                    0 => async move { self.completion_keys(&patterns[0]).await }.boxed(),
                    _ => async move { self.completion_hash_keys(&patterns[0], p).await }.boxed(),
                })
                .collect(),
            "HSCAN" => {
                if !patterns.is_empty() {
                    let ret = vec![async move { self.completion_keys(&patterns[0]).await }.boxed()];
                    ret
                } else {
                    vec![]
                }
            }
            "HSETNX" => {
                let mut ret = Vec::with_capacity(2);
                if !patterns.is_empty() {
                    let x = async move { self.completion_keys(&patterns[0]).await }.boxed();
                    ret.push(x);
                }
                if patterns.len() > 1 {
                    let x =
                        async move { self.completion_hash_keys(&patterns[0], &patterns[1]).await }
                            .boxed();
                    ret.push(x);
                }
                ret
            }
            "HSTRLEN" => {
                let mut ret = Vec::with_capacity(2);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                if patterns.len() > 1 {
                    ret.push(
                        async move { self.completion_hash_keys(&patterns[0], &patterns[1]).await }
                            .boxed(),
                    );
                }
                ret
            }
            "HVALS" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "INCR" | "INCRBY" | "INCRBYFLOAT" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "INFO" => {
                // https://github.com/redis/redis-doc/blob/master/commands/info.md
                // info has no separate documentation for each command
                // so simple command completion will not working
                vec![async move {
                    const INFO_SECTIONS: &[&str] = &[
                        "server",
                        "clients",
                        "memory",
                        "persistence",
                        "stats",
                        "replication",
                        "cpu",
                        "commandstats",
                        "latencystats",
                        "sentinel",
                        "cluster",
                        "modules",
                        "keyspace",
                        "errorstats",
                        "all",
                        "default",
                    ];
                    anyhow::Ok(
                        INFO_SECTIONS
                            .iter()
                            .map(|x| CompletionItem {
                                item: (*x).into(),
                                kind: CompletionKind::Command,
                                desc: None,
                            })
                            .collect::<Vec<_>>()
                            .into(),
                    )
                }
                .boxed()]
            }
            "KEYS" => {
                vec![]
            }
            "LASTSAVE" => {
                vec![]
            }
            "LINDEX" | "LINSERT" | "LLEN" | "LPOP" | "LPUSH" | "LPUSHX" | "LRANGE" | "LREM"
            | "LSET" | "LTRIM" => {
                /*
                命令	作用	示例
                LPUSH	从 左端（头）插入一个或多个元素	`LPUSH q 1 2 3` → `[3,2,1]`
                LPUSHX	仅当 key 存在且是列表时才 左端插入	`LPUSHX q 4`
                RPUSH	从 右端（尾）插入（你没有列出，但对称）	`RPUSH q 4 5` → `[3,2,1,4,5]`
                LPOP	从 左端弹出一个元素	`LPOP q` → `3`，剩余 `[2,1,4,5]`
                RPOP	从 右端弹出（对称）	`RPOP q` → `5`
                LLEN	返回列表长度	`LLEN q` → `4`
                LRANGE	按索引区间 读取元素	`LRANGE q 0 -1` → 全部
                LINDEX	按索引 获取单个元素	`LINDEX q 2` → `1`
                LSET	按索引 修改单个元素	`LSET q 1 99`
                LINSERT	在 指定值前/后插入元素	`LINSERT q BEFORE 99 88`
                LREM	删除 指定值的 N 个元素	`LREM q 2 88` → 删 2 个 88
                LTRIM	只保留索引区间的元素，其余截断	`LTRIM q 0 2`
                */
                let mut ret = Vec::new();
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }

            "MGET" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p.as_str()).await }.boxed())
                .collect(),
            "MOVE" => {
                let mut ret = Vec::new();
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "MSET" => patterns
                .iter()
                .enumerate()
                .map(|(index, key)| {
                    let x = if index % 2 == 0 {
                        async move { self.completion_keys(key).await }.boxed()
                    } else {
                        dummy.clone()
                    };

                    x
                })
                .collect(),
            "MSETNX" => {
                // 但它的特点是在所有指定的键都不存在的情况下，才会执行设置操作。
                // 所以不需要提醒数据库的key
                vec![]
            }
            "OBJECT" => {
                let mut ret = Vec::with_capacity(2);

                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            // redis给我所有OBJECT 的子命令rust数组
                            /// Redis 7.x OBJECT 命令所有子命令
                            pub const OBJECT_SUBCOMMANDS: &[&str] =
                                &["ENCODING", "FREQ", "IDLETIME", "REFCOUNT", "HELP"];
                            anyhow::Ok(
                                OBJECT_SUBCOMMANDS
                                    .iter()
                                    .map(|x| CompletionItem {
                                        item: (*x).into(),
                                        kind: CompletionKind::Command,
                                        desc: None,
                                    })
                                    .collect::<Vec<_>>()
                                    .into(),
                            )
                        }
                        .boxed(),
                    );
                }
                if patterns.len() > 1 {
                    ret.push(async move { self.completion_keys(&patterns[1]).await }.boxed());
                }
                ret
            }
            "PERSIST" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "PEXPIRE" | "PEXPIREAT" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "PFADD" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "PFCOUNT" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "PFMERGE" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p).await }.boxed())
                .collect(),
            "PING" => {
                //
                vec![]
            }
            "PSETEX" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "PTTL" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "PUBLISH" => {
                vec![]
            }
            "RANDOMKEY" => {
                //
                vec![]
            }
            "READONLY" => {
                //
                vec![]
            }
            "READWRITE" => {
                vec![]
            }
            "RENAME" | "RENAMENX" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "RESTORE" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "RPOP" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }

            "RPOPLPUSH" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p).await }.boxed())
                .collect(),

            "RPUSH" | "RPUSHX" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "SADD" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "SCARD" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "SDIFF" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p).await }.boxed())
                .collect(),
            "SDIFFSTORE" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p).await }.boxed())
                .collect(),

            "SETBIT" | "SETEX" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "SETNX" => {
                //
                vec![]
            }
            "SETRANGE" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "SINTER" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p).await }.boxed())
                .collect(),
            "SINTERSTORE" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p).await }.boxed())
                .collect(),
            "SISMEMBER" => {
                let mut ret = Vec::with_capacity(1);

                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                if patterns.len() > 1 {
                    ret.push(
                        async move { self.completion_set_member(&patterns[0], &patterns[1]).await }
                            .boxed(),
                    );
                }
                ret
            }
            "SMEMBERS" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "SMOVE" => patterns
                .iter()
                .enumerate()
                .map(|(index, p)| match index {
                    0 | 1 => async move { self.completion_keys(p).await }.boxed(),
                    2 => {
                        async move { self.completion_set_member(&patterns[0], &patterns[2]).await }
                            .boxed()
                    }
                    _ => dummy.clone(),
                })
                .collect(),

            "SORT" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "SPOP" | "SRANDMEMBER" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "SREM" => patterns
                .iter()
                .enumerate()
                .map(|(index, p)| match index {
                    0 => async move { self.completion_keys(&patterns[0]).await }.boxed(),
                    _ => async move { self.completion_set_member(&patterns[0], p).await }.boxed(),
                })
                .collect(),

            "SSCAN" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "STRLEN" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "SUNION" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p).await }.boxed())
                .collect(),
            "SUNIONSTORE" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p).await }.boxed())
                .collect(),
            "TOUCH" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p).await }.boxed())
                .collect(),
            "TTL" | "TYPE" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "UNLINK" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p).await }.boxed())
                .collect(),

            "ZADD" | "ZCARD" | "ZCOUNT" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "ZINCRBY" => patterns
                .iter()
                .enumerate()
                .map(|(index, p)| match index {
                    0 => async move { self.completion_keys(p).await }.boxed(),
                    1 => dummy.clone(),
                    2 => async move { self.completion_zset_member(&patterns[0], p).await }.boxed(),
                    _ => dummy.clone(),
                })
                .collect(),
            "ZINTERSTORE" | "ZUNIONSTORE" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p).await }.boxed())
                .collect(),
            "ZLEXCOUNT" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "COPY" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p).await }.boxed())
                .collect(),
            "ZREM" => patterns
                .iter()
                .enumerate()
                .map(|(index, p)| match index {
                    0 => async move { self.completion_keys(&patterns[0]).await }.boxed(),
                    _ => async move { self.completion_zset_member(&patterns[0], p).await }.boxed(),
                })
                .collect(),
            "ZRANGE" | "ZRANGEBYLEX" | "ZRANGEBYSCORE" | "ZRANK" | "ZREMRANGEBYLEX"
            | "ZREMRANGEBYRANK" | "ZREMRANGEBYSCORE" | "ZREVRANGE" | "ZREVRANGEBYLEX"
            | "ZREVRANGEBYSCORE" | "ZREVRANK" | "ZSCAN" | "ZSCORE" => {
                /*
                命令	作用	一句话记忆
                `ZRANGE`	按 分数从小到大 取一段排行	正序排行榜
                `ZREVRANGE`	按 分数从大到小 取一段排行	倒序排行榜
                `ZRANK`	查某 member 正序排行（0 开始）	我第几名？
                `ZREVRANK`	查某 member 倒序排行	倒数第几名？
                `ZSCORE`	取某 member 的 分数	多少分？
                `ZRANGEBYSCORE`	按 分数区间 取成员（正序）	90100 分的有谁？
                `ZREVRANGEBYSCORE`	按 分数区间 取成员（倒序）	从高到低 10090 分
                `ZRANGEBYLEX`	按 字典序区间 取成员（分数必须相同）	名字 AC 都有谁？
                `ZREVRANGEBYLEX`	同上，但 倒序字典序	名字 ZX
                `ZREMRANGEBYRANK`	删除 排行区间 内成员	删掉倒数 100 名
                `ZREMRANGEBYSCORE`	删除 分数区间 内成员	清理 060 分的
                `ZREMRANGEBYLEX`	删除 字典序区间 内成员（分数相同）	删掉 name AC
                `ZSCAN`	游标分批遍历 ZSET	大数据下不阻塞
                */
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0]).await }.boxed());
                }
                ret
            }

            _ => {
                vec![]
            }
        };
        anyhow::Ok(fns)
    }
}

#[async_trait::async_trait]
impl LanguageServer for RedisDB {
    async fn ai_text_completion(
        &self,
        text: &str,
        line: u32,
        col: u32,
    ) -> anyhow::Result<CompletionList> {
        let loc = sqlparser::tokenizer::Location::new(line as u64, col as u64);
        for (line_index, text) in text.lines().enumerate() {
            if text.trim().is_empty() {
                continue;
            }
            let tokens = redis_tokenize(text).unwrap_or_default();
            for token in tokens.iter() {
                let line = line_index + 1;
                if in_pos(
                    loc,
                    sqlparser::tokenizer::Location::new(line as u64, (token.start + 1) as u64),
                    token.token.len() as u64,
                ) {
                    // hit  position.
                    const MAX: usize = 200;
                    let mut ret = Vec::with_capacity(MAX);
                    let mut conn = self.get_conn().await?;
                    let mut stream = conn
                        .scan_match::<_, String>(format!("*{}*", token.token))
                        .await?;
                    let mut incomplete = false;
                    while let Some(item) = stream.next().await {
                        let item = item?;
                        if ret.len() >= MAX {
                            incomplete = true;
                            break;
                        }
                        ret.push(CompletionItem {
                            item: item.into(),
                            kind: CompletionKind::Table,
                            desc: None,
                        });
                    }
                    ret.extend(static_redis_command_completion_items().items.into_iter());
                    return anyhow::Ok(CompletionList {
                        items: ret,
                        incomplete,
                        from_dot: false,
                    });
                }
            }
        }

        anyhow::Ok(Default::default())
    }

    fn extract_table_select_selection(&self, _sql: &str) -> anyhow::Result<Option<String>> {
        self.backend_kind().not_support_feature()
    }
    fn extract_table_select_sort(&self, _sql: &str) -> anyhow::Result<Option<String>> {
        self.backend_kind().not_support_feature()
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        anyhow::Ok(get_command_from_sql(sql)?.iter().any(|x| {
            x.first()
                .map(|x| {
                    x.eq_ignore_ascii_case("FLUSHDB")
                        || x.eq_ignore_ascii_case("FLUSHALL")
                        || x.eq_ignore_ascii_case("DEL")
                        || x.eq_ignore_ascii_case("SHUTDOWN")
                        || x.eq_ignore_ascii_case("DEBUG")
                        || x.eq_ignore_ascii_case("REPLICAOF")
                        || x.eq_ignore_ascii_case("SWAPDB")
                        || x.eq_ignore_ascii_case("EVAL")
                        || x.eq_ignore_ascii_case("EVALSHA")
                })
                .unwrap_or(false)
        }))
    }

    async fn auto_completion(
        &self,
        query_id: Option<String>,
        sql: &str,
        line: u32,
        col: u32,
        _use_lexer: bool,
    ) -> anyhow::Result<CompletionList> {
        self.completion_id.set_optional(query_id);
        let pos = sqlparser::tokenizer::Location {
            line: line as u64,
            column: col as u64,
        };
        for (index, sql) in sql.lines().enumerate() {
            let sql = sql.trim();
            if sql.is_empty() {
                continue;
            }
            let tokens = redis_tokenize(sql)?;
            if tokens.is_empty() {
                continue;
            }

            let command = {
                let t = &tokens[0];
                let line = (index + 1) as u64;
                if in_pos(
                    pos,
                    sqlparser::tokenizer::Location {
                        line,
                        column: (t.start + 1) as u64,
                    },
                    t.token.len() as u64,
                ) {
                    return anyhow::Ok(static_redis_command_completion_items());
                }
                t.token.clone().to_uppercase()
            };
            let patterns: Vec<String> = tokens.iter().skip(1).map(|x| x.token.clone()).collect();
            for (token, f) in tokens
                .iter()
                .skip(
                    1, // skip command
                )
                .zip(
                    self.make_auto_completion_fn_by_command(&command, &patterns)?
                        .into_iter(),
                )
            {
                if in_pos(
                    pos,
                    sqlparser::tokenizer::Location {
                        line: (index + 1) as u64,
                        column: (token.start + 1) as u64,
                    },
                    token.token.len() as u64,
                ) {
                    return f.await;
                }
            }
        }
        // lexer command
        let ret = static_redis_command_completion_items();
        anyhow::Ok(ret)
    }
}

pub static REDIS_COMMANDS_MD_FILES: include_dir::Dir =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/../redis-doc/commands");

fn get_commands_from_docs() -> Vec<String> {
    let mut ret = vec![];
    for en in REDIS_COMMANDS_MD_FILES.entries() {
        match en {
            include_dir::DirEntry::Dir(..) => {}
            include_dir::DirEntry::File(file) => {
                let name = file.path().file_name().and_then(|x| x.to_str());
                if let Some(name) = name {
                    if name.ends_with(".md") {
                        if name.starts_with("_index") {
                            continue;
                        }
                        if name.contains("-") {
                            // this is sub command
                            continue;
                        }
                        ret.push(name.strip_suffix(".md").unwrap_or_default().to_uppercase());
                    }
                }
            }
        }
    }
    ret
}

fn is_simple_command(command: &str) -> bool {
    static SIMPLE: &[&str] = &[
        "ACL", "CLIENT", "CLUSTER", "COMMAND", "CONFIG", "FUNCTION", "LATENCY", "MEMORY", "MODULE",
        "PUBSUB", "SCRIPT", "XGROUP", "XINFO", "SLOWLOG",
    ];
    SIMPLE.iter().any(|x| x.eq_ignore_ascii_case(command))
}

fn get_simple_command_sub(command: &str) -> Vec<&'static str> {
    let match_str = format!("{}-", command.to_lowercase());
    let mut ret = vec![];
    for en in REDIS_COMMANDS_MD_FILES.entries() {
        match en {
            include_dir::DirEntry::Dir(..) => {}
            include_dir::DirEntry::File(file) => {
                let name = file.path().file_name().and_then(|x| x.to_str());
                if let Some(name) = name {
                    if name.starts_with(&match_str) {
                        ret.push(&name[match_str.len()..name.len() - 3]);
                    }
                }
            }
        }
    }
    ret
}

fn static_redis_command_completion_items() -> CompletionList {
    static ITEMS: OnceLock<Vec<CompletionItem>> = OnceLock::new();
    ITEMS
        .get_or_init(|| {
            get_commands_from_docs()
                .into_iter()
                .map(|command| CompletionItem {
                    item: command.into(),
                    kind: CompletionKind::Command,
                    desc: None,
                })
                .collect::<Vec<_>>()
        })
        .clone()
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::{QuerySql, SchemaTrait},
        enable_test_log,
        language_server::completion_test::test_file_auto_completion,
        redis::make_test_object,
    };
    use std::path::Path;

    #[tokio::test]
    async fn test_auto_completion() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("redis_auto_completion_test.txt");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();
        let db = make_test_object().await;
        // set up environment
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table("value_string", false).await;
        let _ = db.drop_table("value_hash", false).await;
        let _ = db.drop_table("value_set", false).await;
        let _ = db.drop_table("value_zset", false).await;
        let _ = db.drop_table("value_list", false).await;

        db.execute_sql(query_id, "set value_string xxxxx")
            .await
            .unwrap()
            .unwrap();
        db.execute_sql(query_id, "HSET value_hash key11111 your_value_here")
            .await
            .unwrap()
            .unwrap();
        db.execute_sql(query_id, "SADD value_set member123")
            .await
            .unwrap()
            .unwrap();
        db.execute_sql(query_id, "ZADD value_zset 100 alice")
            .await
            .unwrap()
            .unwrap();
        db.execute_sql(query_id, "lpush value_list 100")
            .await
            .unwrap()
            .unwrap();

        test_file_auto_completion(&content, &db, Some(query_id), false).await;
    }
    #[ignore = "手动跑跑的测试"]
    #[tokio::test]
    async fn test_completion_xxx() {
        enable_test_log();
        let db = make_test_object().await;
        test_file_auto_completion(
            r#"ZRANGE set
11 zset_value"#,
            &db,
            None,
            false,
        )
        .await;
    }

    #[test]
    fn test_get_simple_command_sub() {
        let x = get_simple_command_sub("config");
        for x in x {
            println!("{}", x);
        }
    }

    #[tokio::test]
    async fn test_ai_completion_test() {
        enable_test_log();
        let db = make_test_object().await;
        let x = db.ai_text_completion("str", 1, 2).await.unwrap();
        for i in x.items.iter() {
            println!("{:?}", i);
        }
    }
}
