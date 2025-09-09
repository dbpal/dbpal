use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;
use std::task::Poll;

use futures_core::future::BoxFuture;
use futures_util::{FutureExt, StreamExt};
use redis::{AsyncCommands, AsyncIter};

use crate::language_server::{CompletionItem, CompletionKind, LanguageServer, in_pos};
use crate::redis::{RedisDB, RedisToken, RedisType};
impl RedisDB {
    async fn completion_keys(
        &self, pattern: &str, kind: Option<RedisType>,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        log::trace!("completion on keys pattern:{}", pattern);
        let mut conn = self.get_conn().await?;
        let keys: Vec<Vec<u8>> = conn.keys(format!("*{}*", pattern)).await?;
        let keys = {
            let mut ks = vec![];
            for k in keys {
                match String::from_utf8(k) {
                    Ok(x) => ks.push(x),
                    Err(_) => {}
                }
            }
            ks
        };
        let mut ret = Vec::new();
        for key in keys.into_iter() {
            let type_matched = match kind {
                Some(ty2) => {
                    let ty = Self::get_key_type(&mut conn, &key).await?;
                    ty == ty2
                }
                None => true,
            };
            if type_matched {
                ret.push(CompletionItem {
                    item: key.into(),
                    kind: CompletionKind::Table,
                    desc: None,
                })
            }
        }
        anyhow::Ok(ret)
    }

    async fn completion_hash_keys(
        &self, hash_key: &str, p: &str,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let mut conn = self.get_conn().await?;
        let mut stream: AsyncIter<'_, Vec<u8>> =
            conn.hscan_match(hash_key, format!("*{}*", p)).await?;
        let mut keys = vec![];
        while let Some(k) = stream.next().await {
            match String::from_utf8(k) {
                Ok(x) => keys.push(x),
                Err(_) => {}
            }
        }
        anyhow::Ok(
            keys.into_iter()
                .map(|x| CompletionItem {
                    kind: CompletionKind::Column,
                    item: x.into(),
                    desc: None,
                })
                .collect(),
        )
    }

    async fn completion_set_member(
        &self, key: &str, p: &str,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let mut conn = self.get_conn().await?;
        let mut stream = conn.sscan_match(key, format!("*{}*", p)).await?;
        let mut keys = vec![];
        while let Some(k) = stream.next().await {
            match String::from_utf8(k) {
                Ok(x) => keys.push(x),
                Err(_) => {}
            }
        }
        anyhow::Ok(
            keys.into_iter()
                .map(|x| CompletionItem {
                    kind: CompletionKind::Column,
                    item: x.into(),
                    desc: None,
                })
                .collect(),
        )
    }

    async fn completion_zset_member(
        &self, key: &str, member: &str,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let mut conn = self.get_conn().await?;
        let mut stream: AsyncIter<(Vec<u8>, f64)> =
            conn.zscan_match(key, format!("*{}*", member)).await?;
        let mut keys = vec![];
        while let Some((key, score)) = stream.next().await {
            match String::from_utf8(key) {
                Ok(x) => keys.push((score, x)),
                Err(_) => {}
            }
        }
        anyhow::Ok(
            keys.into_iter()
                .map(|(score, key)| CompletionItem {
                    kind: CompletionKind::Column,
                    item: key.into(),
                    desc: Some(score.to_string().into()),
                })
                .collect(),
        )
    }

    fn make_auto_completion_fn_by_command<'a, 'b>(
        &'a self, command: &str, patterns: &'b [String],
    ) -> anyhow::Result<Vec<BoxFuture<'b, anyhow::Result<Vec<CompletionItem>>>>>
    where
        'a: 'b,
    {
        /// dummy auto completion don't return anything
        #[derive(Clone, Copy, Debug)]
        struct Dummy {}
        impl Future for Dummy {
            type Output = anyhow::Result<Vec<CompletionItem>>;

            fn poll(self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
                Poll::Ready(anyhow::Ok(vec![]))
            }
        }
        let dummy = Box::pin(Dummy {});

        let fns: Vec<BoxFuture<'_, anyhow::Result<Vec<CompletionItem>>>> = match command
            .to_uppercase()
            .as_str()
        {
            "APPEND" | "SET" => {
                let key = patterns.first();
                match key {
                    Some(key) => {
                        vec![
                            async move { self.completion_keys(key, Some(RedisType::String)).await }
                                .boxed(),
                        ]
                    }
                    None => vec![],
                }
            }
            "BGREWRITEAOF" => {
                vec![]
            }
            "BGSAVE" => {
                vec![
                    async move {
                        anyhow::Ok(vec![
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
                        ])
                    }
                    .boxed(),
                ]
            }
            "BITCOUNT" => {
                let key = patterns.first();
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key, None).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }
            "BITFIELD" => {
                let key = patterns.first();
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key, None).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }

            "BITOP" => {
                let mut vs = vec![
                    async move {
                        anyhow::Ok(vec![
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
                        ])
                    }
                    .boxed(),
                ];
                let x = patterns
                    .iter()
                    .skip(1)
                    .map(|x| async move { self.completion_keys(x, None).await }.boxed())
                    .collect::<Vec<_>>();

                vs.extend(x);
                vs
            }
            "BITPOS" => {
                let key = patterns.first();
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key, None).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }
            "BLPOP" | "BRPOP" => patterns
                .iter()
                .map(|key| {
                    async move { self.completion_keys(key, Some(RedisType::List)).await }.boxed()
                })
                .collect::<Vec<_>>(),

            "BRPOPLPUSH" => patterns
                .iter()
                .enumerate()
                .map(|(index, key)| match index {
                    0 | 1 => async move { self.completion_keys(key, Some(RedisType::List)).await }
                        .boxed(),
                    _ => dummy.clone(),
                })
                .collect::<Vec<_>>(),
            "CLIENT" => {
                vec![
                    async move {
                        /*
                           帮我列出所有redis的cliet的子命令用rust数组
                        */
                        const CLIENT_SUBCOMMANDS: &[&str] = &[
                            "LIST",
                            "SETNAME",
                            "GETNAME",
                            "KILL",
                            "ID",
                            "REPLY",
                            "PAUSE",
                            "UNPAUSE",
                            "TRACKING",
                            "TRACKINGINFO",
                            "GETREDIR",
                        ];

                        anyhow::Ok(
                            CLIENT_SUBCOMMANDS
                                .iter()
                                .map(|x| CompletionItem {
                                    item: (*x).into(),
                                    kind: CompletionKind::Command,
                                    desc: None,
                                })
                                .collect::<Vec<_>>(),
                        )
                    }
                    .boxed(),
                ]
            }
            "CLUSTER" => {
                vec![
                    async move {
                        /*
                          帮我列出所有redis的CLUSTER的子命令 rust数组
                        */
                        /// Redis 7.x CLUSTER 命令的全部子命令
                        pub const CLUSTER_SUBCOMMANDS: &[&str] = &[
                            "ADDSLOTS",
                            "ADDSLOTSRANGE",
                            "BUMPEPOCH",
                            "COUNT-FAILURE-REPORTS",
                            "COUNTKEYSINSLOT",
                            "DELSLOTS",
                            "DELSLOTSRANGE",
                            "FAILOVER",
                            "FLUSHSLOTS",
                            "FORGET",
                            "GETKEYSINSLOT",
                            "INFO",
                            "KEYSLOT",
                            "MEET",
                            "MYID",
                            "NODES",
                            "REPLICATE",
                            "RESET",
                            "SAVECONFIG",
                            "SET-CONFIG-EPOCH",
                            "SETSLOT",
                            "SLAVES",
                            "REPLICAS",
                            "SLOTS",
                        ];
                        anyhow::Ok(
                            CLUSTER_SUBCOMMANDS
                                .iter()
                                .map(|x| CompletionItem {
                                    item: (*x).into(),

                                    kind: CompletionKind::Command,
                                    desc: None,
                                })
                                .collect::<Vec<_>>(),
                        )
                    }
                    .boxed(),
                ]
            }

            "CONFIG" => {
                vec![
                    async move {
                        const CONFIG_SUBCOMMANDS: &[&str] =
                            &["GET", "SET", "RESETSTAT", "REWRITE", "HELP"];

                        anyhow::Ok(
                            CONFIG_SUBCOMMANDS
                                .iter()
                                .map(|x| CompletionItem {
                                    item: (*x).into(),

                                    kind: CompletionKind::Command,
                                    desc: None,
                                })
                                .collect::<Vec<_>>(),
                        )
                    }
                    .boxed(),
                ]
            }
            "DECR" | "DECRBY" => {
                let key = patterns.first();
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key, Some(RedisType::String )).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }

            "DEL" => patterns
                .iter()
                .map(|x| async move { self.completion_keys(x, None).await }.boxed())
                .collect(),
            "DUMP" => {
                let key = patterns.first();
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key, None).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }
            "EXISTS" => {
                let x = patterns
                    .iter()
                    .map(|key| async move { self.completion_keys(key, None).await }.boxed())
                    .collect::<Vec<_>>();
                x
            }

            "EXPIRE" | "EXPIREAT" => patterns
                .iter()
                .enumerate()
                .map(|(index, key)| {
                    match index {
                        0 => async move { self.completion_keys(key, None).await }.boxed(),
                        _ => dummy.clone(),
                    }
                    .boxed()
                })
                .collect::<Vec<_>>(),

            "GEOADD" | "GEORADIUS" => {
                // geo data use zset to store.
                if !patterns.is_empty() {
                    vec![
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Zset))
                                .await
                        }
                        .boxed(),
                    ]
                } else {
                    vec![]
                }
            }
            "GEODIST" | "GEOHASH" | "GEOPOS" | "GEORADIUSBYMEMBER" => patterns
                .iter()
                .enumerate()
                .map(|(index, p)| match index {
                    0 => async move {
                        self.completion_keys(&patterns[0], Some(RedisType::Zset))
                            .await
                    }
                    .boxed(),
                    _ => async move { self.completion_zset_member(&patterns[0], p).await }.boxed(),
                })
                .collect(),
            "GET" | "GETBIT" | "GETRANGE" | "GETSET" => {
                if !patterns.is_empty() {
                    vec![
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::String))
                                .await
                        }
                        .boxed(),
                    ]
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
                                async move { self.completion_keys(p, Some(RedisType::Hash)).await }
                                    .boxed()
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
                    vec![
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Hash))
                                .await
                        }
                        .boxed(),
                    ]
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
                                async move { self.completion_keys(p, Some(RedisType::Hash)).await }
                                    .boxed()

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
                    vec![
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Hash))
                                .await
                        }
                        .boxed(),
                    ]
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
                                        self.completion_keys(&patterns[0], Some(RedisType::Hash))
                                            .await
                                    }
                                    .boxed()
                                    // suggest hash key
                                }
                                _ => async move {
                                    self.completion_hash_keys(&patterns[0], &pattern).await
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
                    0 => async move {
                        self.completion_keys(&patterns[0], Some(RedisType::Hash))
                            .await
                    }
                    .boxed(),
                    _ => async move { self.completion_hash_keys(&patterns[0], p).await }.boxed(),
                })
                .collect(),
            "HSCAN" => {
                if !patterns.is_empty() {
                    let ret = vec![
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Hash))
                                .await
                        }
                        .boxed(),
                    ];
                    ret
                } else {
                    vec![]
                }
            }
            "HSETNX" => {
                let mut ret = Vec::with_capacity(2);
                if !patterns.is_empty() {
                    let x = async move {
                        self.completion_keys(&patterns[0], Some(RedisType::Hash))
                            .await
                    }
                    .boxed();
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
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Hash))
                                .await
                        }
                        .boxed(),
                    );
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
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Hash))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "INCR" | "INCRBY" | "INCRBYFLOAT" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::String))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "INFO" => {
                vec![
                    async move {
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
                                .collect(),
                        )
                    }
                    .boxed(),
                ]
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
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::List))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }

            "MGET" => patterns
                .iter()
                .map(|p| {
                    async move {
                        self.completion_keys(p.as_str(), Some(RedisType::String))
                            .await
                    }
                    .boxed()
                })
                .collect(),
            "MOVE" => {
                let mut ret = Vec::new();
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "MSET" => patterns
                .iter()
                .enumerate()
                .map(|(index, key)| {
                    let x = if index % 2 == 0 {
                        async move { self.completion_keys(key, Some(RedisType::String)).await }
                            .boxed()
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
                                    .collect(),
                            )
                        }
                        .boxed(),
                    );
                }
                if patterns.len() > 1 {
                    ret.push(async move { self.completion_keys(&patterns[1], None).await }.boxed());
                }
                ret
            }
            "PERSIST" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "PEXPIRE" | "PEXPIREAT" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "PFADD" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "PFCOUNT" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "PFMERGE" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p, None).await }.boxed())
                .collect(),
            "PING" => {
                //
                vec![]
            }
            "PSETEX" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::String))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "PTTL" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
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
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "RESTORE" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "RPOP" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::List))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }

            "RPOPLPUSH" => patterns
                .iter()
                .map(|p| {
                    async move { self.completion_keys(p, Some(RedisType::List)).await }.boxed()
                })
                .collect(),

            "RPUSH" | "RPUSHX" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::List))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SADD" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Set))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SCARD" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Set))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SCRIPT" => {
                let mut ret = Vec::with_capacity(2);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            // redis给我SCRIPT 的子命令rust数组
                            /// Redis 7.x SCRIPT 命令的全部子命令
                            pub const SCRIPT_SUBCOMMANDS: &[&str] =
                                &["LOAD", "EXISTS", "FLUSH", "KILL", "DEBUG", "HELP"];

                            anyhow::Ok(
                                SCRIPT_SUBCOMMANDS
                                    .iter()
                                    .map(|x| CompletionItem {
                                        item: (*x).into(),
                                        kind: CompletionKind::Command,
                                        desc: None,
                                    })
                                    .collect(),
                            )
                        }
                        .boxed(),
                    );
                }

                ret
            }
            "DEBUG" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            // redis给我debug 的子命令rust数组
                            pub const DEBUG_SUBCOMMANDS: &[&str] = &[
                                "HELP",
                                "SEGFAULT",
                                "OOM",
                                "ASSERT",
                                "RELOAD",
                                "LOADAOF",
                                "OBJECT",
                                "SDSLEN",
                                "POPULATE",
                                "DIGEST",
                                "SLEEP",
                                "LEAK",
                                "STRUCTSIZE",
                                "PAUSE",
                            ];

                            anyhow::Ok(
                                DEBUG_SUBCOMMANDS
                                    .iter()
                                    .map(|x| CompletionItem {
                                        item: (*x).into(),
                                        kind: CompletionKind::Command,
                                        desc: None,
                                    })
                                    .collect(),
                            )
                        }
                        .boxed(),
                    );
                }

                ret
            }
            "SDIFF" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p, Some(RedisType::Set)).await }.boxed())
                .collect(),
            "SDIFFSTORE" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p, Some(RedisType::Set)).await }.boxed())
                .collect(),

            "SETBIT" | "SETEX" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::String))
                                .await
                        }
                        .boxed(),
                    );
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
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::String))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SINTER" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p, Some(RedisType::Set)).await }.boxed())
                .collect(),
            "SINTERSTORE" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p, Some(RedisType::Set)).await }.boxed())
                .collect(),
            "SISMEMBER" => {
                let mut ret = Vec::with_capacity(1);

                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Set))
                                .await
                        }
                        .boxed(),
                    );
                }
                if patterns.len() > 1 {
                    ret.push(
                        async move { self.completion_set_member(&patterns[0], &patterns[1]).await }
                            .boxed(),
                    );
                }
                ret
            }
            "SLOWLOG" => {
                let mut ret = Vec::with_capacity(2);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            pub const SLOWLOG_SUBCOMMANDS: &[&str] = &[
                                "GET",   // SLOWLOG GET [count]
                                "LEN",   // SLOWLOG LEN
                                "RESET", // SLOWLOG RESET
                                "HELP",  // SLOWLOG HELP
                            ];
                            anyhow::Ok(
                                SLOWLOG_SUBCOMMANDS
                                    .iter()
                                    .map(|x| CompletionItem {
                                        item: (*x).into(),
                                        kind: CompletionKind::Command,
                                        desc: None,
                                    })
                                    .collect(),
                            )
                        }
                        .boxed(),
                    );
                }

                ret
            }
            "SMEMBERS" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Set))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SMOVE" => patterns
                .iter()
                .enumerate()
                .map(|(index, p)| match index {
                    0 | 1 => {
                        async move { self.completion_keys(p, Some(RedisType::Set)).await }.boxed()
                    }
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
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "SPOP" | "SRANDMEMBER" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Set))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SREM" => patterns
                .iter()
                .enumerate()
                .map(|(index, p)| match index {
                    0 => async move {
                        self.completion_keys(&patterns[0], Some(RedisType::Set))
                            .await
                    }
                    .boxed(),
                    _ => async move { self.completion_set_member(&patterns[0], p).await }.boxed(),
                })
                .collect(),

            "SSCAN" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Set))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "STRLEN" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::String))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SUNION" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p, Some(RedisType::Set)).await }.boxed())
                .collect(),
            "SUNIONSTORE" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p, Some(RedisType::Set)).await }.boxed())
                .collect(),
            "TOUCH" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p, None).await }.boxed())
                .collect(),
            "TTL" | "TYPE" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "UNLINK" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(p, None).await }.boxed())
                .collect(),

            "ZADD" | "ZCARD" | "ZCOUNT" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Zset))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "ZINCRBY" => patterns
                .iter()
                .enumerate()
                .map(|(index, p)| match index {
                    0 => {
                        async move { self.completion_keys(p, Some(RedisType::Zset)).await }.boxed()
                    }
                    1 => dummy.clone(),
                    2 => async move { self.completion_zset_member(&patterns[0], p).await }.boxed(),
                    _ => dummy.clone(),
                })
                .collect(),
            "ZINTERSTORE" | "ZUNIONSTORE" => patterns
                .iter()
                .map(|p| {
                    async move { self.completion_keys(p, Some(RedisType::Zset)).await }.boxed()
                })
                .collect(),
            "ZLEXCOUNT" => {
                let mut ret = Vec::with_capacity(1);
                if !patterns.is_empty() {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Zset))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "ZREM" => patterns
                .iter()
                .enumerate()
                .map(|(index, p)| match index {
                    0 => async move {
                        self.completion_keys(&patterns[0], Some(RedisType::Zset))
                            .await
                    }
                    .boxed(),
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
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Zset))
                                .await
                        }
                        .boxed(),
                    );
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
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, _use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let pos = sqlparser::tokenizer::Location {
            line: line as u64,
            column: col as u64,
        };
        for (index, sql) in sql.lines().enumerate() {
            let sql = sql.trim();
            if sql.is_empty() {
                continue;
            }
            let tokens = RedisToken::tokenize(sql, (index + 1) as u64)?;
            if tokens.is_empty() {
                continue;
            }
            let command = match &tokens[0] {
                RedisToken::NotString {
                    value,
                    start_column,
                    line,
                    ..
                } => {
                    if in_pos(
                        pos,
                        sqlparser::tokenizer::Location {
                            line: *line,
                            column: *start_column as u64,
                        },
                        value.len() as u64,
                    ) {
                        return anyhow::Ok(static_redis_command_completion_items());
                    }
                    value.clone().to_uppercase()
                }
                _ => {
                    continue;
                }
            };
            let patterns: Vec<String> = tokens.iter().skip(1).map(|x| x.to_string()).collect();
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
                match token {
                    RedisToken::NotString {
                        start_column,
                        value,
                        ..
                    } => {
                        println!(
                            "pos:{:?} {:?} {:?} {:?}",
                            pos,
                            sqlparser::tokenizer::Location {
                                line: (index + 1) as u64,
                                column: (*start_column) as u64,
                            },
                            value.len(),
                            token
                        );

                        if in_pos(
                            pos,
                            sqlparser::tokenizer::Location {
                                line: (index + 1) as u64,
                                column: (*start_column) as u64,
                            },
                            value.len() as u64,
                        ) {
                            return f.await;
                        }
                    }
                    _ => {
                        continue;
                    }
                }
            }
        }
        // lexer command
        let ret = static_redis_command_completion_items();
        anyhow::Ok(ret)
    }
}

// redis给我一个列表一些特殊的字符串在redis中有些特殊含义比如nx 之类的但是都不在命令列表里  完整rust数组呢
/// 所有在 Redis 命令参数中拥有特殊含义的非命令字符串
pub const REDIS_SPECIAL_ARGS: &[&str] = &[
    // SET / SETNX / SETEX / PSETEX 等
    "NX",      // 仅当 key 不存在时才操作
    "XX",      // 仅当 key 已存在时才操作
    "EX",      // 过期时间（秒）
    "PX",      // 过期时间（毫秒）
    "EXAT",    // 绝对 Unix 时间戳（秒）
    "PXAT",    // 绝对 Unix 时间戳（毫秒）
    "KEEPTTL", // 保留原 TTL（覆盖时不重置过期时间）
    "GET",     // SET 时返回旧值
    // SORT 命令
    "ASC",
    "DESC",
    "ALPHA",
    // LIMIT 子句（SORT / ZINTERSTORE / ZUNIONSTORE 等）
    "LIMIT",
    // BITFIELD / BITFIELD_RO
    "WRAP",
    "SAT",
    "FAIL",
    // CLIENT KILL
    "SKIPME",
    // ACL SETUSER
    "ON",
    "OFF",
    "RESET",
    "ALLKEYS",
    "ALLCOMMANDS",
    "NOPASS",
    // CLUSTER 相关
    "HARD",
    "SOFT",
    // RESTORE
    "REPLACE",
    "ABSTTL",
    // EXPIRE / EXPIREAT / PEXPIRE / PEXPIREAT
    "GT", // 仅当新 TTL 大于当前 TTL 时生效
    "LT", // 仅当新 TTL 小于当前 TTL 时生效
    "NX", // 仅当 key 没有 TTL 时才设置
    "XX", // 仅当 key 已有 TTL 时才设置
    // SCRIPT DEBUG
    "YES",
    "SYNC",
    "NO",
    // ACL LOG
    "RESET",
    // ACL DELUSER / DELUSER
    "FORCE",
    // ACL LIST
    "USERS",
    // OBJECT
    "REFCOUNT",
    "ENCODING",
    "IDLETIME",
    "FREQ",
    // CLIENT REPLY
    "ON",
    "OFF",
    "SKIP",
    // CONFIG GET/SET
    "GET",
    "SET",
    "RESETSTAT",
    "REWRITE",
    // SLOWLOG
    "GET",
    "LEN",
    "RESET",
    // MIGRATE
    "COPY",
    "REPLACE",
    "AUTH",
    "AUTH2",
    "KEYS",
    // RESTORE-ASKING 内部标记
    "ASKING",
    // PUB/SUB
    "CHANNELS",
    "NUMSUB",
    "NUMPAT",
    // MEMORY
    "USAGE",
    "STATS",
    "PURGE",
    "MALLOC-STATS",
    "DOCTOR",
    // LATENCY
    "LATEST",
    "HISTORY",
    "GRAPH",
    "RESET",
    "DOCTOR",
    "HELP",
    "WITH",
    "SCORE",
];

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{
        db::{QuerySql, SchemaTrait},
        enable_test_log,
        language_server::completion_test::test_file_auto_completion,
        redis::make_test_object,
    };

    #[tokio::test]
    async fn test_auto_completion() {
        enable_test_log();
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("redis_auto_completion_test.txt");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();
        let db = make_test_object().await;
        // set up environment
        let query_id = db.open_connection().await.unwrap();

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

        test_file_auto_completion(&content, &db, false).await;
    }
    #[ignore = "手动跑跑的测试"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_completion_xxx() {
        enable_test_log();
        let db = make_test_object().await;
        test_file_auto_completion(
            r#"ZRANGE set
11 zset_value"#,
            &db,
            false,
        )
        .await;
    }
}

#[derive(Debug)]
pub struct RedisDialet {}

impl sqlparser::dialect::Dialect for RedisDialet {
    /// Determine if a character is a valid start character for an unquoted
    /// identifier
    fn is_identifier_start(&self, ch: char) -> bool {
        /*
        Redis 的键（key）可以包含多种字符，具体包括：
        字母：大写字母（A-Z）和小写字母（a-z）
        数字：0-9
        特殊字符：如下划线（_）、冒号（:）、破折号（-）、点（.）、斜杠（/），以及其他大多数 ASCII 字符。
        不过，建议避免使用空格和某些特殊字符（如空字符、控制字符等）作为键的一部分，以免造成潜在的问题。同时，键的长度限制为 512 MB，但通常建议保持键的简短和有意义。
                */
        ch.is_alphabetic()
            || ch == ':'
            || ch == '$'
            || ch == '/'
            || ch.is_numeric()
            || ch == '.'
            || ch == '-'
            || ch == '_'
    }

    fn supports_string_literal_backslash_escape(&self) -> bool {
        true
    }

    fn supports_unicode_string_literal(&self) -> bool {
        true
    }

    /// Determine if a character is a valid unquoted identifier character
    fn is_identifier_part(&self, ch: char) -> bool {
        self.is_identifier_start(ch)
    }
}

fn static_redis_command_completion_items() -> Vec<CompletionItem> {
    static ITEMS: OnceLock<Vec<CompletionItem>> = OnceLock::new();
    ITEMS
        .get_or_init(|| {
            REDIS_COMMANDS
                .iter()
                .chain(REDIS_SPECIAL_ARGS.iter())
                .map(|x| CompletionItem {
                    item: (*x).into(),
                    kind: CompletionKind::Command,
                    desc: None,
                })
                .collect::<Vec<_>>()
        })
        .clone()
}

pub const REDIS_COMMANDS: &[&str] = &[
    "APPEND",
    "ASKING",
    "AUTH",
    "BGREWRITEAOF",
    "BGSAVE",
    "BITCOUNT",
    "BITFIELD",
    "BITOP",
    "BITPOS",
    "BLPOP",
    "BRPOP",
    "BRPOPLPUSH",
    "CLIENT",
    "CLUSTER",
    "CONFIG",
    "DBSIZE",
    "DEBUG",
    "DECR",
    "DECRBY",
    "DEL",
    "DISCARD",
    "DUMP",
    "ECHO",
    "EVAL",
    "EVALSHA",
    "EXEC",
    "EXISTS",
    "EXPIRE",
    "EXPIREAT",
    "FLUSHALL",
    "FLUSHDB",
    "GEOADD",
    "GEODIST",
    "GEOHASH",
    "GEOPOS",
    "GEORADIUS",
    "GEORADIUSBYMEMBER",
    "GET",
    "GETBIT",
    "GETRANGE",
    "GETSET",
    "HDEL",
    "HEXISTS",
    "HGET",
    "HGETALL",
    "HINCRBY",
    "HINCRBYFLOAT",
    "HKEYS",
    "HLEN",
    "HMGET",
    "HMSET",
    "HSCAN",
    "HSET",
    "HSETNX",
    "HSTRLEN",
    "HVALS",
    "INCR",
    "INCRBY",
    "INCRBYFLOAT",
    "INFO",
    "KEYS",
    "LASTSAVE",
    "LINDEX",
    "LINSERT",
    "LLEN",
    "LPOP",
    "LPUSH",
    "LPUSHX",
    "LRANGE",
    "LREM",
    "LSET",
    "LTRIM",
    "MGET",
    "MIGRATE",
    "MOVE",
    "MSET",
    "MSETNX",
    "MULTI",
    "OBJECT",
    "PERSIST",
    "PEXPIRE",
    "PEXPIREAT",
    "PFADD",
    "PFCOUNT",
    "PFMERGE",
    "PING",
    "PSETEX",
    "PTTL",
    "PUBLISH",
    "RANDOMKEY",
    "READONLY",
    "READWRITE",
    "RENAME",
    "RENAMENX",
    "RESTORE",
    "ROLE",
    "RPOP",
    "RPOPLPUSH",
    "RPUSH",
    "RPUSHX",
    "SADD",
    "SAVE",
    "SCAN",
    "SCARD",
    "SCRIPT",
    "SDIFF",
    "SDIFFSTORE",
    "SET",
    "SETBIT",
    "SETEX",
    "SETNX",
    "SETRANGE",
    "SHUTDOWN",
    "SINTER",
    "SINTERSTORE",
    "SISMEMBER",
    "SLAVEOF",
    "SLOWLOG",
    "SMEMBERS",
    "SMOVE",
    "SORT",
    "SPOP",
    "SRANDMEMBER",
    "SREM",
    "SSCAN",
    "STRLEN",
    "SUNION",
    "SUNIONSTORE",
    "SYNC",
    "TIME",
    "TOUCH",
    "TTL",
    "TYPE",
    "UNLINK",
    "ZADD",
    "ZCARD",
    "ZCOUNT",
    "ZINCRBY",
    "ZINTERSTORE",
    "ZLEXCOUNT",
    "ZRANGE",
    "ZRANGEBYLEX",
    "ZRANGEBYSCORE",
    "ZRANK",
    "ZREM",
    "ZREMRANGEBYLEX",
    "ZREMRANGEBYRANK",
    "ZREMRANGEBYSCORE",
    "ZREVRANGE",
    "ZREVRANGEBYLEX",
    "ZREVRANGEBYSCORE",
    "ZREVRANK",
    "ZSCAN",
    "ZSCORE",
    "ZUNIONSTORE",
];
