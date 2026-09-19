use serde::Deserialize;

use crate::db::models;
use crate::domain::{ContentSection, MediaPlaceholder};
use crate::error::AppError;

const SEED_TOML: &str = include_str!("../../data/seed.toml");

#[derive(Debug, Deserialize)]
struct SeedFile {
    #[serde(default)]
    projects: Vec<SeedProject>,
    #[serde(default)]
    articles: Vec<SeedArticle>,
}

#[derive(Debug, Deserialize)]
struct SeedProject {
    slug: String,
    title: String,
    category: String,
    summary: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    sections: Vec<ContentSection>,
    hero: Option<MediaPlaceholder>,
    has_detail_page: bool,
    featured: bool,
    sort_order: i64,
    #[serde(default)]
    related_slugs: Vec<String>,
    repo_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SeedArticle {
    slug: String,
    title: String,
    category: String,
    summary: String,
    featured: bool,
    sort_order: i64,
    #[serde(default)]
    sections: Vec<ContentSection>,
}

pub async fn seed_if_empty(db: &mut toasty::Db) -> Result<(), AppError> {
    if !models::Project::all().exec(&mut *db).await?.is_empty() {
        return Ok(());
    }

    let seed: SeedFile = toml::from_str(SEED_TOML)?;

    for project in seed.projects {
        toasty::create!(models::Project {
            slug: project.slug,
            title: project.title,
            category: project.category,
            summary: project.summary,
            tags: serde_json::to_string(&project.tags)?,
            sections: serde_json::to_string(&project.sections)?,
            hero: project
                .hero
                .map(|value| serde_json::to_string(&value))
                .transpose()?,
            has_detail_page: project.has_detail_page,
            featured: project.featured,
            sort_order: project.sort_order,
            repo_url: project.repo_url,
            related_slugs: serde_json::to_string(&project.related_slugs)?,
        })
        .exec(&mut *db)
        .await?;
    }

    for article in seed.articles {
        toasty::create!(models::Article {
            slug: article.slug,
            title: article.title,
            category: article.category,
            summary: article.summary,
            status: "draft".to_owned(),
            featured: article.featured,
            sort_order: article.sort_order,
            published_at: None,
            reading_time_minutes: None,
            sections: serde_json::to_string(&article.sections)?,
        })
        .exec(&mut *db)
        .await?;
    }

    Ok(())
}
