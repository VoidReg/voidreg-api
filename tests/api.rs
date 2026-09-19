use actix_web::http::StatusCode;
use actix_web::test;
use serde_json::json;

use voidreg_api::app::build;
use voidreg_api::config::Config;
use voidreg_api::db::Db;

async fn setup_db() -> Db {
    let db = Db::memory().await.expect("in-memory turso");
    db.migrate().await.expect("migrate");
    db.seed_if_empty().await.expect("seed");
    db
}

#[actix_web::test]
async fn healthz_ok() {
    let app = test::init_service(build(setup_db().await, Config::for_test())).await;
    let req = test::TestRequest::get().uri("/healthz").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn readyz_ok() {
    let app = test::init_service(build(setup_db().await, Config::for_test())).await;
    let req = test::TestRequest::get().uri("/readyz").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn lists_projects_and_featured_subset() {
    let app = test::init_service(build(setup_db().await, Config::for_test())).await;

    let req = test::TestRequest::get()
        .uri("/api/v1/projects")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    let items = body["items"].as_array().expect("items");
    assert_eq!(items.len(), 4);
    assert_eq!(items[0]["slug"], "minirt");
    assert!(items[0]["sections"].as_array().unwrap().is_empty());

    let req = test::TestRequest::get()
        .uri("/api/v1/projects?featured=true")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    let items = body["items"].as_array().expect("items");
    assert_eq!(items.len(), 3);
    assert!(items.iter().all(|item| item["featured"] == true));
}

#[actix_web::test]
async fn gets_project_by_slug_and_404() {
    let app = test::init_service(build(setup_db().await, Config::for_test())).await;

    let req = test::TestRequest::get()
        .uri("/api/v1/projects/minirt")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["slug"], "minirt");
    assert_eq!(body["has_detail_page"], true);
    assert!(!body["sections"].as_array().unwrap().is_empty());
    assert_eq!(body["related_slugs"][0], "webserv");

    let req = test::TestRequest::get()
        .uri("/api/v1/projects/missing")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn lists_and_gets_blogs() {
    let app = test::init_service(build(setup_db().await, Config::for_test())).await;

    let req = test::TestRequest::get().uri("/api/v1/blogs").to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 3);

    let req = test::TestRequest::get()
        .uri("/api/v1/blogs?featured=true")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 2);

    let req = test::TestRequest::get()
        .uri("/api/v1/blogs/how-to-validate-a-semantic-color-system")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["status"], "draft");
    assert!(body["sections"].as_array().unwrap().len() > 1);
}

#[actix_web::test]
async fn contact_validates_and_accepts() {
    let app = test::init_service(build(setup_db().await, Config::for_test())).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/contact")
        .set_json(json!({ "name": "", "email": "bad", "message": "" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let req = test::TestRequest::post()
        .uri("/api/v1/contact")
        .set_json(json!({
            "name": "Ada",
            "email": "ada@example.com",
            "message": "Hello from the scaffold tests."
        }))
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["status"], "received");
}

#[actix_web::test]
async fn newsletter_is_idempotent() {
    let app = test::init_service(build(setup_db().await, Config::for_test())).await;

    let payload = json!({ "email": "reader@example.com" });
    let req = test::TestRequest::post()
        .uri("/api/v1/newsletter/subscribe")
        .set_json(&payload)
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["status"], "subscribed");

    let req = test::TestRequest::post()
        .uri("/api/v1/newsletter/subscribe")
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let req = test::TestRequest::post()
        .uri("/api/v1/newsletter/subscribe")
        .set_json(json!({ "email": "not-an-email" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
