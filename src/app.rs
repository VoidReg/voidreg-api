use actix_cors::Cors;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceFactory, ServiceRequest, ServiceResponse};
use actix_web::http::header::{self, HeaderName};
use actix_web::middleware::{Compress, Logger};
use actix_web::{App, Error, web};

use crate::api::v1;
use crate::auth::rate::RateLimiter;
use crate::config::Config;
use crate::db::Db;

pub fn build(
    db: Db,
    config: Config,
) -> App<
    impl ServiceFactory<
        ServiceRequest,
        Config = (),
        Response = ServiceResponse<impl MessageBody>,
        Error = Error,
        InitError = (),
    >,
> {
    let cors_origins = config.cors_origins.clone();
    let app = App::new()
        .app_data(web::JsonConfig::default().limit(7 * 1024 * 1024))
        .app_data(web::Data::new(db))
        .app_data(web::Data::new(config))
        // Each worker keeps its own limiter, so limits are approximate under
        // multiple workers; see auth::rate.
        .app_data(web::Data::new(RateLimiter::new()))
        .wrap(Logger::default())
        .wrap(build_cors(&cors_origins))
        .wrap(Compress::default())
        .service(v1::health::healthz)
        .service(v1::health::readyz)
        .service(v1::sitemap::sitemap_blogs)
        .service(web::scope("/api/v1").configure(v1::configure));

    #[cfg(feature = "swagger")]
    let app = {
        use utoipa::OpenApi;
        use utoipa_swagger_ui::SwaggerUi;

        use crate::openapi::ApiDoc;

        app.service(
            SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", ApiDoc::openapi()),
        )
    };

    app
}

fn build_cors(origins: &[String]) -> Cors {
    if origins.iter().any(|origin| origin == "*") {
        return Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);
    }

    let mut cors = Cors::default()
        .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
        .allowed_headers(vec![
            header::AUTHORIZATION,
            header::ACCEPT,
            header::CONTENT_TYPE,
            header::IF_NONE_MATCH,
            HeaderName::from_static("x-csrf-token"),
        ])
        .expose_headers(vec![header::ETAG])
        .supports_credentials()
        .max_age(3600);

    for origin in origins {
        cors = cors.allowed_origin(origin);
    }

    cors
}
