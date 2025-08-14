use std::net::SocketAddr;

use actix_web::{App, HttpServer, dev::Server, middleware, web, web::Data};
use config_parser::config::ServerConfig;
use tracing::instrument;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::routes;

#[instrument(skip())]
pub async fn run_server(addr_to_bind: SocketAddr, config: ServerConfig) -> crate::error::Result<Server> {
    #[derive(OpenApi)]
    #[openapi(
        paths(crate::routes::health::handle, crate::routes::exec_test::handle),
        components(schemas(
            crate::routes::exec_test::ExecuteTestRequest,
            crate::routes::exec_test::ExecuteTestResponse
        ))
    )]
    struct ApiDoc;

    let config = Data::new(config);
    let actix_server = HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(tracing_actix_web::TracingLogger::default())
            .service(web::resource("/health").route(web::get().to(routes::health::handle)))
            .service(web::resource("/exec_test").route(web::post().to(routes::exec_test::handle)))
            .app_data(config.clone())
            .service(SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", ApiDoc::openapi()))
    })
    .bind(addr_to_bind)?;
    Ok(actix_server.run())
}
