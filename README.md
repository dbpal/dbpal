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



redis分页实现要好好研究下
oracle add table column有问题

检查default value

781128




###可以把日志文件变大一点
perl -e 'print "hello world\n" x 1024 for 1..2048' > dbpal.log



这个导致linux的title不对呢
https://github.com/tauri-apps/tauri/issues/13749




CONFIG HELP redis返回的值不对啊


列出重复的依赖
cargo tree -d




 npx eslint src --ext .js,.jsx,.ts,.tsx --fix



 

 SELECT
    *
from
    has_data_2,
    Persons ,
    has_data_1  





统计
~~~bash 

find . -path ./target -prune -o -name '*.rs' -exec wc -l {} +

~~~


#C7C0B4 


some-couchbase password
test1234


couchbase create bucket


docker run  --name some-neo4j  -d     \
    --publish=7474:7474 --publish=7687:7687 \
    neo4j
 默认密码是和用户名一样 我改成test1234
。



~~~
解决没有签名的问题
xattr -d com.apple.quarantine /Applications/dbpal.app  
~~~


保存不显示的列m


https://api.openai-proxy.org/v1




// api key sk-2zegNGK2WhxZ9jToc17JBp260Z5vD5Cdd4zXG4s4WISRyczO
// https://api.moonshot.cn/v1







UPDATE "information_schema"."sql_features" SET "sub_feature_name" = NULL,"sub_feature_id" = NULL,"comments" = NULL WHERE  "comments" = '' AND  "sub_feature_id" = '' AND  "feature_name" = 'Embedded PL/I' AND  "sub_feature_name" = '' AND  "is_supported" = 'NO' AND  "is_verified_by" IS NULL AND  "feature_id" = 'B017' 这个语句做了啥啊


这个 SQL UPDATE 语句是在更新 information_schema.sql_features 表中的一条记录，但看起来是在将字段值清空为 NULL。


pg好像会返回Opiton为null但是返回了Some("") 导致pg的一个前端页面没有修改的东西 也出现了提示用户修改