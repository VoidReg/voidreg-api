use actix_web::web::ServiceConfig;

pub mod blogs;
pub mod contact;
pub mod health;
pub mod newsletter;
pub mod projects;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(projects::list_projects)
        .service(projects::get_project)
        .service(blogs::list_blogs)
        .service(blogs::get_blog)
        .service(contact::create_contact)
        .service(newsletter::subscribe);
}
