use utoipa::OpenApi;

use crate::api::v1;
use crate::domain::{
    AdminLoginRequest, AdminSessionResponse, Article, ArticleListResponse, ArticleMutationResponse,
    ArticleStatus, ArticleWriteRequest, CommentAdminListResponse, CommentAdminView,
    CommentCreateRequest, CommentCreateResponse, CommentListResponse, CommentPublic,
    CommenterIdentity, CommenterSessionResponse, ContactRequest, ContactResponse, ContentSection,
    HealthResponse, MediaPlaceholder, NewsletterRequest, NewsletterResponse, Project,
    ProjectListResponse,
};
use crate::error::{ErrorBody, ErrorDetail};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Voidreg API",
        version = "0.2.0",
        description = "Backend for the Voidreg site: projects, blogs, comments, contact, newsletter, and admin publishing."
    ),
    paths(
        v1::health::healthz,
        v1::health::readyz,
        v1::projects::list_projects,
        v1::projects::get_project,
        v1::blogs::list_blogs,
        v1::blogs::get_blog,
        v1::comments::list_comments,
        v1::comments::create_comment,
        v1::contact::create_contact,
        v1::newsletter::subscribe,
        v1::auth::providers,
        v1::auth::start,
        v1::auth::callback,
        v1::auth::show_session,
        v1::auth::clear_session,
        v1::admin::session::login,
        v1::admin::session::show,
        v1::admin::session::logout,
        v1::admin::articles::list_articles,
        v1::admin::articles::create_article,
        v1::admin::articles::get_article,
        v1::admin::articles::update_article,
        v1::admin::articles::publish_article,
        v1::admin::articles::unpublish_article,
        v1::admin::articles::delete_article,
        v1::admin::comments::list_comments,
        v1::admin::comments::approve_comment,
        v1::admin::comments::delete_comment
    ),
    components(schemas(
        HealthResponse,
        ProjectListResponse,
        Project,
        ArticleListResponse,
        Article,
        ArticleStatus,
        ContentSection,
        MediaPlaceholder,
        ContactRequest,
        ContactResponse,
        NewsletterRequest,
        NewsletterResponse,
        CommentCreateRequest,
        CommentCreateResponse,
        CommentListResponse,
        CommentPublic,
        CommentAdminListResponse,
        CommentAdminView,
        CommenterIdentity,
        CommenterSessionResponse,
        AdminLoginRequest,
        AdminSessionResponse,
        ArticleWriteRequest,
        ArticleMutationResponse,
        ErrorBody,
        ErrorDetail
    )),
    tags(
        (name = "health", description = "Liveness and readiness"),
        (name = "projects", description = "Portfolio projects"),
        (name = "blogs", description = "Published engineering notes"),
        (name = "comments", description = "Reader comments"),
        (name = "auth", description = "Commenter OAuth identity"),
        (name = "admin", description = "Admin publishing and moderation"),
        (name = "contact", description = "Contact form"),
        (name = "newsletter", description = "Newsletter subscriptions")
    )
)]
pub struct ApiDoc;
