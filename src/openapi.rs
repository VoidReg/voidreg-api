use utoipa::OpenApi;

use crate::api::v1;
use crate::domain::{
    Article, ArticleListResponse, ArticleStatus, ContactRequest, ContactResponse, ContentSection,
    HealthResponse, MediaPlaceholder, NewsletterRequest, NewsletterResponse, Project,
    ProjectListResponse,
};
use crate::error::{ErrorBody, ErrorDetail};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Voidreg API",
        version = "0.1.0",
        description = "Backend for the Voidreg site: projects, blogs, contact, and newsletter."
    ),
    paths(
        v1::health::healthz,
        v1::health::readyz,
        v1::projects::list_projects,
        v1::projects::get_project,
        v1::blogs::list_blogs,
        v1::blogs::get_blog,
        v1::contact::create_contact,
        v1::newsletter::subscribe
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
        ErrorBody,
        ErrorDetail
    )),
    tags(
        (name = "health", description = "Liveness and readiness"),
        (name = "projects", description = "Portfolio projects"),
        (name = "blogs", description = "Engineering notes"),
        (name = "contact", description = "Contact form"),
        (name = "newsletter", description = "Newsletter subscriptions")
    )
)]
pub struct ApiDoc;
