//! Blog sitemap fragment. nginx routes /sitemap-blogs.xml on the edge to
//! this endpoint because the static site cannot enumerate dynamic slugs.

use actix_web::HttpResponse;
use actix_web::get;
use actix_web::web::Data;

use crate::config::Config;
use crate::db::Db;
use crate::error::AppError;

#[get("/sitemap-blogs.xml")]
pub async fn sitemap_blogs(db: Data<Db>, config: Data<Config>) -> Result<HttpResponse, AppError> {
    let articles = db.list_published_articles(None).await?;
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for article in &articles {
        xml.push_str("  <url><loc>");
        xml.push_str(&format!(
            "{}/blog/{}",
            config.public_base_url.trim_end_matches('/'),
            xml_escape(&article.slug)
        ));
        xml.push_str("</loc>");
        if let Some(published_at) = &article.published_at {
            xml.push_str(&format!("<lastmod>{}</lastmod>", xml_escape(published_at)));
        }
        xml.push_str("</url>\n");
    }
    xml.push_str("</urlset>\n");

    Ok(HttpResponse::Ok()
        .content_type("application/xml; charset=utf-8")
        .body(xml))
}

fn xml_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}
