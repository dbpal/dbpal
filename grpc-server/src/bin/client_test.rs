// 在 src/main.rs 中添加此代码以实现客户端
pub mod my_service {
    tonic::include_proto!("app");
}

use my_service::{my_service_client::MyServiceClient, HelloRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = MyServiceClient::connect("http://[::1]:50051").await?;
    let request = tonic::Request::new(HelloRequest {
        name: "World".into(),
    });

    let response = client.say_hello(request).await?;
    println!("RESPONSE={:?}", response);
    let request = tonic::Request::new(HelloRequest {
        name: "World".into(),
    });
    let response = client.return_err(request).await.unwrap();
    println!("RESPONSE={:?}", response);
    Ok(())
}
