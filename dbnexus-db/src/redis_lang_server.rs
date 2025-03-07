use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::task::Poll;

use futures_core::future::BoxFuture;
use futures_util::FutureExt;
use redis::AsyncCommands;
use sqlparser::tokenizer::Token;
use tokio::sync::Mutex;

use crate::language_server::{
    CompletionItem, CompletionKind, LanguageServer, in_pos, match_pattern_and_sort,
};
use crate::redis::{RedisDB, RedisType};
impl RedisDB {
    async fn completion_keys(
        &self, pattern: &str, kind: Option<RedisType>,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        log::trace!("completion on keys pattern:{}", pattern);
        let mut conn = self.get_conn().await?;
        let keys: Vec<String> = conn.keys(format!("*{}*", pattern)).await?;
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

    async fn completion_hash_keys(&self, key: &str) -> anyhow::Result<Vec<CompletionItem>> {
        let mut conn = self.get_conn().await?;
        let keys: Vec<String> = conn.hkeys(key).await?;
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
                let key = patterns.get(0);
                match key {
                    Some(key) => {
                        vec![
                        async move { self.completion_keys(&key, Some(RedisType::String)).await }
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
                let key = patterns.get(0);
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key, None).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }
            "BITFIELD" => {
                let key = patterns.get(0);
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
                        ])
                    }
                    .boxed(),
                ];
                let x = patterns
                    .iter()
                    .skip(1)
                    .map(|x| async move { self.completion_keys(&x, None).await }.boxed())
                    .collect::<Vec<_>>();

                vs.extend(x.into_iter());
                vs
            }
            "BITPOS" => {
                let key = patterns.get(0);
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key, None).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }

            "BLPOP" | "BRPOP" => {
                let x = patterns
                    .iter()
                    .map(|key| {
                        async move { self.completion_keys(&key, Some(RedisType::List)).await }
                            .boxed()
                    })
                    .collect::<Vec<_>>();
                x
            }

            "BRPOPLPUSH" => {
                let key = patterns.get(0);
                let key = match key {
                    Some(key) => {
                        let key = key.to_string();
                        async move { self.completion_keys(&key, Some(RedisType::List)).await }
                            .boxed()
                    }
                    None => dummy.clone(),
                };
                let key2 = patterns.get(1);

                let key2 = match key2 {
                    Some(key) => {
                        let key = key.to_string();
                        async move { self.completion_keys(&key, Some(RedisType::List)).await }
                            .boxed()
                    }
                    None => dummy.clone(),
                };
                vec![key, key2]
            }
            "CLIENT" => {
                vec![
                    async move {
                        anyhow::Ok(vec![
                            // 客户端连接管理
                            CompletionItem {
                                item: "CONNECT".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "DISCONNECT".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "KILL".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "LIST".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            // 客户端信息获取
                            CompletionItem {
                                item: "GETNAME".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "ID".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "ADDR".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "PORT".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            //  客户端连接配置
                            CompletionItem {
                                item: "SETNAME".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "PAUSE".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "REPLY".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            // 客户端连接监控
                            CompletionItem {
                                item: "LIST".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "TRACKING".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                        ])
                    }
                    .boxed(),
                ]
            }
            "CLUSTER" => {
                vec![
                    async move {
                        anyhow::Ok(vec![
                            // 集群创建和配置
                            CompletionItem {
                                item: "CREATE".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "MEET".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "ADDSLOTS".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "REPLICATE".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            // 集群状态查询
                            CompletionItem {
                                item: "NODES".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "INFO".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "KEYSLOT".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "COUNTKEYSINSLOT".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            // 集群维护和修复
                            CompletionItem {
                                item: "FAILOVER".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "FORGET".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "FLUSHSLOTS".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "RESET".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            // 集群扩展和缩容
                            CompletionItem {
                                item: "ADDSLOTS".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "DELSLOTS".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "SETSLOT".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            // 其他集群操作
                            CompletionItem {
                                item: "SAVECONFIG".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "SLOTS".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "TALK".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                        ])
                    }
                    .boxed(),
                ]
            }
            "CONFIG" => {
                vec![
                    async move {
                        anyhow::Ok(vec![
                            CompletionItem {
                                item: "GET".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "SET".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "RESETSTAT".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "REWRITE".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                        ])
                    }
                    .boxed(),
                ]
            }
            "DECR" | "DECRBY" => {
                let key = patterns.get(0);
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
                .map(|x| async move { self.completion_keys(&x, None).await }.boxed())
                .collect(),
            "DUMP" => {
                let key = patterns.get(0);
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
                    .map(|key| async move { self.completion_keys(&key, None).await }.boxed())
                    .collect::<Vec<_>>();
                x
            }

            "EXPIRE" | "EXPIREAT" => {
                let mut count = 0;
                let x = patterns
                    .iter()
                    .map(|key| {
                        let x = async move { self.completion_keys(&key, None).await }.boxed();
                        count += 1;
                        x
                    })
                    .collect::<Vec<_>>();
                x
            }
            "GEOADD" | "GEODIST" | "GEOHASH" | "GEOPOS" | "GEORADIUS" | "GEORADIUSBYMEMBER" => {
                // geo data use zset to store.
                if patterns.len() > 0 {
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
            "GET" | "GETBIT" | "GETRANGE" | "GETSET" => {
                if patterns.len() > 0 {
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
                            1 => {
                                let key = &patterns[0];
                                // hash key store in pattern[0]
                                async move { self.completion_hash_keys(key.as_str()).await }.boxed()
                            }
                            _ => dummy.clone(),
                        }
                    })
                    .collect()
            }
            "HGETALL" => {
                if patterns.len() > 0 {
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
                                async move { self.completion_hash_keys(key.as_str()).await }.boxed()
                            }
                            _ => dummy.clone(),
                        }
                    })
                    .collect()
            }
            "HKEYS" | "HLEN" => {
                if patterns.len() > 0 {
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
                // get key first.
                let hash_keys: Arc<Mutex<Option<Vec<CompletionItem>>>> = Default::default();
                let hash_keys: Vec<_> = (0..patterns.len()).map(|_| hash_keys.clone()).collect();

                patterns
                    .iter()
                    .enumerate()
                    .zip(hash_keys.into_iter())
                    .map(|((index, p), hash_keys)| {
                        match index {
                            0 => {
                                async move { self.completion_keys(&p, Some(RedisType::Hash)).await }
                                    .boxed()
                                // suggest hash key
                            }
                            _ => {
                                // hash key store in pattern[0]
                                async move {
                                    let mut hash_keys = hash_keys.lock().await;
                                    if hash_keys.is_none() {
                                        *hash_keys =
                                            Some(self.completion_hash_keys(&patterns[0]).await?);
                                    }

                                    anyhow::Ok(hash_keys.clone().unwrap_or(vec![]))
                                        as anyhow::Result<Vec<CompletionItem>>
                                }
                                .boxed()
                            }
                        }
                    })
                    .collect()
            }
            "HMSET" | "HSET" => {
                if patterns.len() > 0 {
                    let mut ret = vec![
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Hash))
                                .await
                        }
                        .boxed(),
                    ];
                    let hash_keys: Arc<Mutex<Option<Vec<CompletionItem>>>> = Default::default();
                    let hash_keys: Vec<_> =
                        (0..patterns.len() - 1).map(|_| hash_keys.clone()).collect();
                    let mut count = 0;
                    patterns.iter().skip(1).zip(hash_keys.into_iter()).for_each(
                        |(_p, hash_keys)| {
                            let x = if count % 2 == 0 {
                                async move {
                                    let mut hash_keys = hash_keys.lock().await;
                                    if hash_keys.is_none() {
                                        *hash_keys =
                                            Some(self.completion_hash_keys(&patterns[0]).await?);
                                    }

                                    anyhow::Ok(hash_keys.clone().unwrap_or(vec![]))
                                        as anyhow::Result<Vec<CompletionItem>>
                                }
                                .boxed()
                            } else {
                                dummy.clone()
                            };
                            ret.push(x);
                            count += 1;
                        },
                    );
                    ret
                } else {
                    vec![]
                }
            }
            "HSCAN" => {
                if patterns.len() > 0 {
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
                if patterns.len() > 0 {
                    let x = async move {
                        self.completion_keys(&patterns[0], Some(RedisType::Hash))
                            .await
                    }
                    .boxed();
                    ret.push(x);
                }
                if patterns.len() > 1 {
                    let x = async move { self.completion_hash_keys(&patterns[0]).await }.boxed();
                    ret.push(x);
                }
                ret
            }
            "HSTRLEN" => {
                let mut ret = Vec::with_capacity(2);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Hash))
                                .await
                        }
                        .boxed(),
                    );
                }
                if patterns.len() > 1 {
                    ret.push(async move { self.completion_hash_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "HVALS" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
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
                if patterns.len() > 0 {
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
                        anyhow::Ok(vec![
                            CompletionItem {
                                item: "SERVER".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "MEMORY".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "PERSISTENCE".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "STATS".into(),
                                desc: None,

                                kind: CompletionKind::Text,
                            },
                            //
                            CompletionItem {
                                item: "REPLICATION".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "CLIENTS".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "CPU".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "COMMANDSTATS".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            //
                            CompletionItem {
                                item: "CLUSTER".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                            CompletionItem {
                                item: "KEYSPACE".into(),
                                kind: CompletionKind::Text,
                                desc: None,
                            },
                        ])
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
                let mut ret = Vec::new();
                if patterns.len() > 0 {
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
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "MSET" => {
                let mut count = 0;
                patterns
                    .iter()
                    .map(|key| {
                        let x = if count % 2 == 0 {
                            async move { self.completion_keys(&key, Some(RedisType::String)).await }
                                .boxed()
                        } else {
                            dummy.clone()
                        };
                        count += 1;
                        x
                    })
                    .collect()
            }
            "MSETNX" => {
                // 但它的特点是在所有指定的键都不存在的情况下，才会执行设置操作。
                // 所以不需要提醒数据库的key
                vec![]
            }
            "OBJECT" => {
                let mut ret = Vec::with_capacity(2);

                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            anyhow::Ok(vec![
                                CompletionItem {
                                    item: "ENCODING".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                                CompletionItem {
                                    item: "IDLETIME".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                                CompletionItem {
                                    item: "REFCOUNT".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                            ])
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
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "PEXPIRE" | "PEXPIREAT" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "PFADD" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "PFCOUNT" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "PFMERGE" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(&p, None).await }.boxed())
                .collect(),
            "PING" => {
                //
                vec![]
            }
            "PSETEX" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
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
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "PUBLISH" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
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
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "RESTORE" | "RESTORE_ASKING" => {
                vec![]
            }
            "RPOP" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
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

            "RPOPLPUSH" => {
                let mut ret = Vec::with_capacity(2);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::List))
                                .await
                        }
                        .boxed(),
                    );
                }
                if patterns.len() > 1 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[1], Some(RedisType::List))
                                .await
                        }
                        .boxed(),
                    );
                }

                ret
            }

            "RPUSH" | "RPUSHX" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
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
                if patterns.len() > 0 {
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
                if patterns.len() > 0 {
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
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            anyhow::Ok(vec![
                                CompletionItem {
                                    item: "LOAD".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                                CompletionItem {
                                    item: "EXISTS".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                                CompletionItem {
                                    item: "FLUSH".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                                CompletionItem {
                                    item: "KILL".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                                CompletionItem {
                                    item: "DEBUG".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                            ])
                        }
                        .boxed(),
                    );
                }

                ret
            }
            "DEBUG" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            anyhow::Ok(vec![
                                CompletionItem {
                                    item: "OBJECT".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                                CompletionItem {
                                    item: "SEGFAULT".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                                CompletionItem {
                                    item: "RELOAD".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                                CompletionItem {
                                    item: "SLEEP".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                                CompletionItem {
                                    item: "HELP".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                            ])
                        }
                        .boxed(),
                    );
                }

                ret
            }
            "SDIFF" => patterns
                .iter()
                .map(|p| {
                    async move { self.completion_keys(&p, Some(RedisType::Set)).await }.boxed()
                })
                .collect(),
            "SDIFFSTORE" => patterns
                .iter()
                .map(|p| {
                    async move { self.completion_keys(&p, Some(RedisType::Set)).await }.boxed()
                })
                .collect(),

            "SETBIT" | "SETEX" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
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
                if patterns.len() > 0 {
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
                .map(|p| {
                    async move { self.completion_keys(&p, Some(RedisType::Set)).await }.boxed()
                })
                .collect(),
            "SINTERSTORE" => patterns
                .iter()
                .map(|p| {
                    async move { self.completion_keys(&p, Some(RedisType::Set)).await }.boxed()
                })
                .collect(),
            "SISMEMBER" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
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
            "SLOWLOG" => {
                let mut ret = Vec::with_capacity(2);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            anyhow::Ok(vec![
                                CompletionItem {
                                    item: "GET".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                                CompletionItem {
                                    item: "LEN".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                                CompletionItem {
                                    item: "RESET".into(),
                                    kind: CompletionKind::Text,
                                    desc: None,
                                },
                            ])
                        }
                        .boxed(),
                    );
                }

                ret
            }
            "SMEMBERS" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
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
            "SMOVE" => {
                let mut ret = Vec::with_capacity(2);
                if patterns.len() > 0 {
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
                        async move {
                            self.completion_keys(&patterns[1], Some(RedisType::Set))
                                .await
                        }
                        .boxed(),
                    );
                }

                ret
            }
            "SORT" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "SPOP" | "SRANDMEMBER" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
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
            "SREM" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
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
            "SSCAN" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
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
                if patterns.len() > 0 {
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
                .map(|p| {
                    async move { self.completion_keys(&p, Some(RedisType::Set)).await }.boxed()
                })
                .collect(),
            "SUNIONSTORE" => patterns
                .iter()
                .map(|p| {
                    async move { self.completion_keys(&p, Some(RedisType::Set)).await }.boxed()
                })
                .collect(),
            "TOUCH" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(&p, None).await }.boxed())
                .collect(),
            "TTL" | "TYPE" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "UNLINK" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(&p, None).await }.boxed())
                .collect(),

            "ZADD" | "ZCARD" | "ZCOUNT" | "ZINCRBY" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
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
            "ZINTERSTORE" => {
                // TODO
                vec![]
            }
            "ZLEXCOUNT" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
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
            "ZRANGE" | "ZRANGEBYLEX" | "ZRANGEBYSCORE" | "ZRANK" | "ZREM" | "ZREMRANGEBYLEX"
            | "ZREMRANGEBYRANK" | "ZREMRANGEBYSCORE" | "ZREVRANGE" | "ZREVRANGEBYLEX"
            | "ZREVRANGEBYSCORE" | "ZREVRANK" | "ZSCAN" | "ZSCORE" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
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
            "ZUNIONSTORE" => {
                // TODO
                vec![]
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
        &self, _uuid: uuid::Uuid, sql: &str, line: u32, col: u32, _use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        for (index, sql) in sql.lines().enumerate() {
            if sql.trim().is_empty() {
                continue;
            }
            let line_token = index + 1;
            let mut tokens = sqlparser::tokenizer::Tokenizer::new(&RedisDialet {}, sql)
                .tokenize_with_location()?;

            // correct the line
            tokens.iter_mut().for_each(|x| {
                x.span.start.line = line_token as u64;
                x.span.end.line = line_token as u64;
            });

            if tokens.len() == 0 {
                continue;
            }

            let tokens = tokens
                .into_iter()
                .filter(|x| match &x.token {
                    Token::Whitespace(_) => false,
                    _ => true,
                })
                .collect::<Vec<_>>();
            if tokens.len() == 0 {
                continue;
            }
            let pos = sqlparser::tokenizer::Location {
                line: line as u64,
                column: col as u64,
            };
            let command = match &tokens[0].token {
                Token::Word(word) => {
                    if in_pos(pos, tokens[0].span.start, word.value.len() as u64) {
                        // we are auto completion on command.
                        let x = match_pattern_and_sort(
                            static_redis_command_completion_items(),
                            &word.value,
                        );
                        return anyhow::Ok(x);
                    }
                    word.value.clone()
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
                match &token.token {
                    Token::Word(word) => {
                        if in_pos(pos, token.span.start, word.value.len() as u64) {
                            let item = f.await;
                            return item.map(|item| match_pattern_and_sort(item, &word.value));
                        }
                    }
                    _ => {
                        continue;
                    }
                }
            }
        }
        // lexer command
        let ret = REDIS_COMMANDS
            .iter()
            .map(|x| CompletionItem {
                item: (*x).into(),
                kind: CompletionKind::Command,
                desc: None,
            })
            .collect::<Vec<_>>();

        anyhow::Ok(ret)
    }
}

#[derive(Debug)]
pub struct RedisDialet {}

impl sqlparser::dialect::Dialect for RedisDialet {
    /// Determine if a character is a valid start character for an unquoted
    /// identifier
    fn is_identifier_start(&self, ch: char) -> bool {
        // https://blog.csdn.net/csbysj2020/article/details/139585685
        ch.is_alphabetic()
            || ch == ':'
            || ch == '$'
            || ch == '/'
            || ch.is_numeric()
            || ch == '.'
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
                .map(|x| CompletionItem {
                    item: (*x).into(),
                    kind: CompletionKind::Command,
                    desc: None,
                })
                .collect::<Vec<_>>()
        })
        .clone()
}

pub const REDIS_COMMANDS: &'static [&'static str] = &[
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
    "RESTORE_ASKING",
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
