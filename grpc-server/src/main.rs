use std::sync::Arc;

use app::my_service_server::{MyService, MyServiceServer};
use app::{HelloRequest, HelloResponse};
use dbnexus_db::app::{App, DbConfigLoader, FileJsonSaver};
use dbnexus_db::init_log;
use grpc_server::app::app_service_server::{AppService, AppServiceServer};
use grpc_server::app::{self, DbConnectionConfig};
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

pub struct AppServiceImpl {
    app: App,
}

#[tonic::async_trait]
impl AppService for AppServiceImpl {
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

fn anyhow_error_to_status(err: anyhow::Error) -> Status {
    Status::with_details(
        Code::Internal,
        format!("{}", err),
        bytes::Bytes::copy_from_slice(format!("{:?}", err).as_str().as_bytes()),
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_log();

    let addr = "[::1]:50051".parse()?;
    let my_service = MyServiceImpl::default();
    let loader: Arc<dyn DbConfigLoader> = Arc::new(FileJsonSaver::new("dbnexus.json").unwrap());
    let app = App::new(loader);

    Server::builder()
        .add_service(MyServiceServer::new(my_service))
        .add_service(AppServiceServer::new(AppServiceImpl { app }))
        .serve(addr)
        .await?;

    Ok(())
}
