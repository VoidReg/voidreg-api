use actix_web::web::ServiceConfig;

pub mod admin;
pub mod auth;
pub mod blogs;
pub mod comments;
pub mod contact;
pub mod health;
pub mod mcp;
pub mod newsletter;
pub mod projects;
pub mod sitemap;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(
        actix_web::web::scope("/mcp")
            .wrap(actix_web::middleware::DefaultHeaders::new().add(("Cache-Control", "no-store")))
            .configure(mcp::configure),
    )
    .service(projects::list_projects)
    .service(projects::get_project)
    .service(blogs::list_blogs)
    .service(blogs::get_blog)
    .service(comments::list_comments)
    .service(comments::list_replies)
    .service(comments::create_comment)
    .service(contact::create_contact)
    .service(newsletter::subscribe)
    .service(auth::providers)
    .service(auth::start)
    .service(auth::callback)
    .service(auth::show_session)
    .service(auth::clear_session)
    .service(
        actix_web::web::scope("/admin")
            .wrap(actix_web::middleware::DefaultHeaders::new().add(("Cache-Control", "no-store")))
            .configure(admin::configure),
    );
}
