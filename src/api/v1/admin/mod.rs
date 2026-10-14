use actix_web::web::ServiceConfig;

pub mod articles;
pub mod comments;
pub mod session;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(session::login)
        .service(session::show)
        .service(session::logout)
        .service(articles::list_articles)
        .service(articles::create_article)
        .service(articles::get_article)
        .service(articles::update_article)
        .service(articles::publish_article)
        .service(articles::unpublish_article)
        .service(articles::delete_article)
        .service(comments::list_comments)
        .service(comments::approve_comment)
        .service(comments::delete_comment);
}
