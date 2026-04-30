use seaweedfs_k8s_operator::protos::GetFilerConfigurationRequest;
use seaweedfs_k8s_operator::protos::seaweed_filer_client::SeaweedFilerClient;

#[tokio::main]
async fn main() {
    let mut client = SeaweedFilerClient::connect("http://127.0.0.1:8889")
        .await
        .unwrap();

    let request = tonic::Request::new(GetFilerConfigurationRequest {});

    let response = client.get_filer_configuration(request).await.unwrap();

    println!("RESPONSE={response:#?}");

}
