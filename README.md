# dbnexus




## how to run dbnexus-app 
~~~
cd dbnexus-app
pnpm run dev
cargo r


 

 

~~~


 SELECT *
FROM V$RESERVED_WORDS;

怎么去获取这么多内置的可以查询的对象
 
table function 是啥啊




PostgreSQL 外部表（Foreign Table） 是 “看起来像本地表，实际数据却存放在别的系统里” 的一张虚拟表。
你可以对它做 SELECT、JOIN、INSERT、UPDATE、DELETE，但数据不保存在 PostgreSQL 本地，而是由外部数据源（另一个数据库、文件、对象存储、API…）实时提供。

拉oracle数据库要用美国的代理


openPath for open export file




sqlite virtual table
use blew code to test
~~~
CREATE VIRTUAL TABLE documents USING fts5(title, content); 


INSERT INTO documents (title, content) VALUES ('First Document', 'This is the content of the first document.');
INSERT INTO documents (title, content) VALUES ('Second Document', 'This is the content of the second document.');
~~~

