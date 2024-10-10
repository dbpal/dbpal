use std::sync::Arc;

use app_grpc_def::my_service_server::{MyService, MyServiceServer};
use app_grpc_def::{HelloRequest, HelloResponse};
use dbnexus_db::app::{App, DbConfigLoader, FileJsonSaver};
use dbnexus_db::enable_test_log;
use grpc_server::app_grpc_def::catelog_service_server::{CatelogService, CatelogServiceServer};
use grpc_server::app_grpc_def::config_service_server::{ConfigService, ConfigServiceServer};
use grpc_server::app_grpc_def::database_service_server::{DatabaseService, DatabaseServiceServer};
use grpc_server::app_grpc_def::schema_service_server::{SchemaService, SchemaServiceServer};
use grpc_server::app_grpc_def::{self, DbConnectionConfig};
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
        let r = request.into_inner();

        panic!("111111111111111")
    }
}

pub struct ConfigServiceImpl {
    app: App,
}

#[tonic::async_trait]
impl ConfigService for ConfigServiceImpl {
    async fn add_config(
        &self, request: Request<DbConnectionConfig>,
    ) -> Result<Response<()>, Status> {
        let request: DbConnectionConfig = request.into_inner();
        let id = uuid::Uuid::new_v4();
        self.app
            .add_config(id.to_string().as_str(), request.into())
            .await
            .map_err(anyhow_error_to_status)?;
        Ok(Response::new(()))
    }
}

#[tonic::async_trait]
impl CatelogService for CatelogServiceImpl {}

#[tonic::async_trait]
impl DatabaseService for DatabaseServiceImpl {}

#[tonic::async_trait]
impl SchemaService for SchemaServiceImpl {}

pub struct DatabaseServiceImpl {
    app: App,
}
pub struct SchemaServiceImpl {
    app: App,
}

pub struct CatelogServiceImpl {
    app: App,
}

fn anyhow_error_to_status(err: anyhow::Error) -> Status {
    Status::with_details(
        Code::Internal,
        format!("{}", err),
        bytes::Bytes::copy_from_slice(format!("{:?}", err).as_str().as_bytes()),
    )
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
        .add_service(CatelogServiceServer::new(CatelogServiceImpl {
            app: app.clone(),
        }))
        .add_service(DatabaseServiceServer::new(DatabaseServiceImpl {
            app: app.clone(),
        }))
        .add_service(SchemaServiceServer::new(SchemaServiceImpl {
            app: app.clone(),
        }))
        .serve(addr)
        .await?;

    Ok(())
}
