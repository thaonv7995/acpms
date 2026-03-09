use acpms_server::api::openapi_spec::ApiDoc;
use utoipa::OpenApi;

fn main() {
    let document = ApiDoc::openapi();
    println!(
        "{}",
        serde_json::to_string_pretty(&document).expect("OpenAPI document should serialize")
    );
}
