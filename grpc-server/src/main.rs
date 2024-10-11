use std::sync::Arc;

use app_grpc_def::my_service_server::{MyService, MyServiceServer};
use app_grpc_def::{HelloRequest, HelloResponse};
use bytes::Bytes;
use dbnexus_db::app::{App, DbConfigLoader, FileJsonSaver};

use grpc_server::app_grpc_def::config_service_server::{ConfigService, ConfigServiceServer};
use grpc_server::app_grpc_def::database_service_server::{DatabaseService, DatabaseServiceServer};

use grpc_server::app_grpc_def::{
    self, DbConnectionConfig, DelConfigReq, DeleteReq, DuplicateTableReq, FormatSqlReq, InsertReq,
    ListAllConfigResp, ListTableResp, NamedStaffVec, OnlyUuidReq, OpendKindResp, QuerySqlReq,
    QuerySqlResp, RedisListTableResp, RedisMatchKeysResp, RenameTableReq, ShowColumnsResp,
    ShowIndexResp, ShowTriggerReq, SqlExecuteRet, StringResp, TableListResp, TableViewListReq,
    TriggerDetail, UpdateConfig, UpdateReq, UserSettings, UuidAndName, UuidAndPattern,
    UuidAndQueryId,
};
use grpc_server::app_grpc_def::{AddConfigResp, TestConnResp};
use tonic::Code;
use tonic::{transport::Server, Request, Response, Status};
#[derive(Default)]
pub struct MyServiceImpl;

#[tonic::async_trait]
impl MyService for MyServiceImpl {
    async fn say_hello(
        &self, request: Request<HelloRequest>,
    ) -> Result<Response<HelloResponse>, Status> {
        let reply = HelloResponse {
            message: format!("Hello, {}!", request.into_inner().name),
        };
        Ok(Response::new(reply))
    }

    async fn return_err(
        &self, request: Request<HelloRequest>,
    ) -> Result<Response<HelloResponse>, Status> {
        Err(Status::with_details(
            Code::Internal,
            "aaaaaaaaaaaaaaaaaaaaaaaa",
            Bytes::copy_from_slice("backtrace!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!".as_bytes()),
        ))
    }
}

pub struct ConfigServiceImpl {
    app: App,
}

#[tonic::async_trait]
impl ConfigService for ConfigServiceImpl {
    async fn add(
        &self, request: Request<DbConnectionConfig>,
    ) -> Result<Response<AddConfigResp>, Status> {
        let request: DbConnectionConfig = request.into_inner();
        let id = uuid::Uuid::new_v4();
        self.app
            .add_config(id.to_string().as_str(), request.into())
            .await
            .map_err(anyhow_error_to_status)?;
        Ok(Response::new(AddConfigResp {
            uuid: id.to_string(),
        }))
    }
    async fn update(
        &self, request: tonic::Request<UpdateConfig>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let request = request.into_inner();
        self.app
            .update_config(&request.uuid, request.config.clone().unwrap().into())
            .await
            .map_err(anyhow_error_to_status)?;

        Ok(Response::new(()))
    }

    async fn del(
        &self, request: tonic::Request<DelConfigReq>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let request = request.into_inner();
        self.app
            .delete_config(&request.uuid)
            .await
            .map_err(anyhow_error_to_status)?;
        Ok(Response::new(()))
    }
    async fn list_all(
        &self, request: tonic::Request<()>,
    ) -> std::result::Result<tonic::Response<ListAllConfigResp>, tonic::Status> {
        let resp = self
            .app
            .loader
            .list_all_configs_cloned()
            .await
            .map_err(anyhow_error_to_status)?;
        Ok(Response::new(resp.into()))
    }

    async fn open(
        &self, request: tonic::Request<OnlyUuidReq>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let request = request.into_inner();
        self.app
            .open(&request.uuid)
            .await
            .map_err(anyhow_error_to_status)?;
        Ok(Response::new(()))
    }
    async fn test_conn(
        &self, request: tonic::Request<OnlyUuidReq>,
    ) -> std::result::Result<tonic::Response<TestConnResp>, tonic::Status> {
        let request = request.into_inner();

        unimplemented!()
    }
}

#[tonic::async_trait]
impl DatabaseService for DatabaseServiceImpl {
    async fn list_table(
        &self, request: tonic::Request<OnlyUuidReq>,
    ) -> std::result::Result<tonic::Response<ListTableResp>, tonic::Status> {
        let request = request.into_inner();
        let resp = self
            .app
            .list_table(&request.uuid)
            .await
            .map_err(anyhow_error_to_status)?;

        Ok(Response::new(ListTableResp {
            tables: resp.into_iter().map(|x| x.into()).collect(),
        }))
    }

    async fn redis_list_table(
        &self, request: tonic::Request<OnlyUuidReq>,
    ) -> std::result::Result<tonic::Response<RedisListTableResp>, tonic::Status> {
        let request = request.into_inner();
        let resp = self
            .app
            .redis_list_table(&request.uuid)
            .await
            .map_err(anyhow_error_to_status)?;

        Ok(Response::new(resp.into()))
    }
    async fn redis_match_keys(
        &self, request: tonic::Request<UuidAndPattern>,
    ) -> std::result::Result<tonic::Response<RedisMatchKeysResp>, tonic::Status> {
        let request = request.into_inner();
        let resp = self
            .app
            .redis_match_keys(&request.uuid, &request.pattern)
            .await
            .map_err(anyhow_error_to_status)?;
        Ok(Response::new(RedisMatchKeysResp { keys: resp }))
    }
    async fn show_columns(
        &self, request: tonic::Request<UuidAndName>,
    ) -> std::result::Result<tonic::Response<ShowColumnsResp>, tonic::Status> {
        let request = request.into_inner();
        let resp = self
            .app
            .show_columns(&request.uuid, &request.name)
            .await
            .map_err(anyhow_error_to_status)?;

        Ok(Response::new(ShowColumnsResp {
            columns: resp.into_iter().map(|x| x.into()).collect(),
        }))
    }

    async fn show_talbe(
        &self, request: tonic::Request<TableViewListReq>,
    ) -> std::result::Result<tonic::Response<TableListResp>, tonic::Status> {
        let request = request.into_inner();
        let resp = self
            .app
            .show_table(
                &request.uuid,
                &request.table_name,
                request.where_str.as_ref().map(|x| x.as_str()),
                request.order_by.as_ref().map(|x| x.as_str()),
                request.page as usize,
                request.page_size as usize,
            )
            .await
            .map_err(anyhow_error_to_status)?;
        Ok(Response::new(resp.into()))
    }

    async fn show_index(
        &self, request: tonic::Request<UuidAndName>,
    ) -> std::result::Result<tonic::Response<ShowIndexResp>, tonic::Status> {
        let request = request.into_inner();
        let resp = self
            .app
            .show_indexes(&request.uuid, &request.name)
            .await
            .map_err(anyhow_error_to_status)?;
        Ok(Response::new(ShowIndexResp {
            indexes: resp.into_iter().map(|x| x.into()).collect(),
        }))
    }

    async fn insert(
        &self, request: tonic::Request<InsertReq>,
    ) -> std::result::Result<tonic::Response<SqlExecuteRet>, tonic::Status> {
        let request = request.into_inner();
        let InsertReq {
            uuid,
            table_name,
            values,
        } = request;

        let resp = self
            .app
            .insert(&uuid, &table_name, values.unwrap().into())
            .await
            .map_err(anyhow_error_to_status)?;
        Ok(Response::new(resp.into()))
    }

    async fn update(
        &self, request: tonic::Request<UpdateReq>,
    ) -> std::result::Result<tonic::Response<SqlExecuteRet>, tonic::Status> {
        let request = request.into_inner();
        let UpdateReq {
            uuid,
            table_name,
            values,
            wheres,
        } = request;
        let resp = self
            .app
            .update(
                &uuid,
                &table_name,
                values.unwrap().into(),
                wheres.unwrap().into(),
            )
            .await
            .map_err(anyhow_error_to_status)?;
        Ok(Response::new(resp.into()))
    }
    async fn truncate_table(
        &self, request: tonic::Request<UuidAndName>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let request = request.into_inner();
        let resp = self
            .app
            .truncate_table(&request.uuid, &request.name)
            .await
            .map_err(anyhow_error_to_status)?;
        Ok(Response::new(resp.into()))
    }
    async fn delete(
        &self, request: tonic::Request<DeleteReq>,
    ) -> std::result::Result<tonic::Response<SqlExecuteRet>, tonic::Status> {
        unimplemented!()
    }
    async fn list_view(
        &self, request: tonic::Request<OnlyUuidReq>,
    ) -> std::result::Result<tonic::Response<NamedStaffVec>, tonic::Status> {
        unimplemented!()
    }
    async fn drop_view(
        &self, request: tonic::Request<UuidAndName>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        unimplemented!()
    }
    async fn drop_table(
        &self, request: tonic::Request<UuidAndName>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        unimplemented!()
    }
    async fn show_view(
        &self, request: tonic::Request<TableViewListReq>,
    ) -> std::result::Result<tonic::Response<TableListResp>, tonic::Status> {
        unimplemented!()
    }
    async fn open_schema(
        &self, request: tonic::Request<UuidAndName>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        unimplemented!()
    }
    async fn open_db(
        &self, request: tonic::Request<UuidAndName>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        unimplemented!()
    }
    async fn opened_kind(
        &self, request: tonic::Request<OnlyUuidReq>,
    ) -> std::result::Result<tonic::Response<OpendKindResp>, tonic::Status> {
        unimplemented!()
    }
    async fn query_sql(
        &self, request: tonic::Request<QuerySqlReq>,
    ) -> std::result::Result<tonic::Response<QuerySqlResp>, tonic::Status> {
        unimplemented!()
    }
    async fn query_sql_open_connection(
        &self, request: tonic::Request<OnlyUuidReq>,
    ) -> std::result::Result<tonic::Response<StringResp>, tonic::Status> {
        unimplemented!()
    }
    async fn query_sql_close_connection(
        &self, request: tonic::Request<UuidAndQueryId>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        unimplemented!()
    }
    async fn query_sql_close_re_connection(
        &self, request: tonic::Request<UuidAndQueryId>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        unimplemented!()
    }
    async fn save_user_settings(
        &self, request: tonic::Request<UserSettings>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        unimplemented!()
    }
    async fn get_user_settings(
        &self, request: tonic::Request<()>,
    ) -> std::result::Result<tonic::Response<UserSettings>, tonic::Status> {
        unimplemented!()
    }
    async fn export_table(
        &self, request: tonic::Request<UuidAndName>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        unimplemented!()
    }
    async fn format_sql(
        &self, request: tonic::Request<FormatSqlReq>,
    ) -> std::result::Result<tonic::Response<StringResp>, tonic::Status> {
        unimplemented!()
    }
    async fn list_trigger(
        &self, request: tonic::Request<UuidAndName>,
    ) -> std::result::Result<tonic::Response<NamedStaffVec>, tonic::Status> {
        unimplemented!()
    }
    async fn show_trigger(
        &self, request: tonic::Request<ShowTriggerReq>,
    ) -> std::result::Result<tonic::Response<TriggerDetail>, tonic::Status> {
        unimplemented!()
    }
    async fn rename_table(
        &self, request: tonic::Request<RenameTableReq>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        unimplemented!()
    }
    async fn duplicate_table(
        &self, request: tonic::Request<DuplicateTableReq>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        unimplemented!()
    }
}

pub struct DatabaseServiceImpl {
    app: App,
}

fn anyhow_error_to_status(err: anyhow::Error) -> Status {
    let ret = Status::with_details(
        Code::Internal,
        format!("{}", err),
        bytes::Bytes::copy_from_slice(format!("{:?}", err).as_str().as_bytes()),
    );
    ret
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let addr = "[::1]:50051".parse()?;
    let my_service = MyServiceImpl::default();
    let loader: Arc<dyn DbConfigLoader> = Arc::new(FileJsonSaver::new("dbnexus.json").unwrap());
    let app = App::new(loader);

    Server::builder()
        .add_service(MyServiceServer::new(my_service))
        .add_service(ConfigServiceServer::new(ConfigServiceImpl {
            app: app.clone(),
        }))
        .add_service(DatabaseServiceServer::new(DatabaseServiceImpl {
            app: app.clone(),
        }))
        .serve(addr)
        .await?;

    Ok(())
}
