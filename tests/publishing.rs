use actix_web::{
    cookie::Cookie,
    http::{StatusCode, header},
    test,
};
use serde_json::{Value, json};
use voidreg_api::{app::build, config::Config, db::Db};

async fn database() -> Db {
    let db = Db::memory().await.unwrap();
    db.migrate().await.unwrap();
    db.migrate().await.unwrap();
    db.seed_if_empty().await.unwrap();
    db
}
fn document(text: &str) -> Value {
    json!({"schemaVersion":1,"blocks":[{"id":"018fabc0-0000-4000-8000-000000000001","type":"publisle:paragraph","schemaVersion":1,"data":{"content":[{"type":"text","value":text}]}}]})
}
fn compiled() -> Value {
    json!({"format":"publisle:publication","formatVersion":1,"identity":"test","rendererBuild":"1","html":"<p>Hello</p>","styles":[],"modules":[],"islands":[],"diagnostics":[]})
}

#[actix_web::test]
async fn publishing_lifecycle_cache_csrf_and_moderation() {
    let app = test::init_service(build(database().await, Config::for_test())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/articles")
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let req = test::TestRequest::post()
        .uri("/api/v1/admin/session")
        .set_json(json!({"password":"voidreg-test-admin"}))
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookies: Vec<Cookie<'static>> = response
        .response()
        .cookies()
        .map(|c| c.into_owned())
        .collect();
    let session = cookies
        .iter()
        .find(|c| c.name() == "voidreg_admin_session")
        .unwrap()
        .clone();
    let csrf = cookies
        .iter()
        .find(|c| c.name() == "voidreg_admin_csrf")
        .unwrap()
        .value()
        .to_owned();
    assert_eq!(session.http_only(), Some(true));
    let payload = json!({"slug":"test-article","title":"Hello","category":"Systems","summary":"Summary","document":document("Hello"),"document_version":1,"compiled":compiled()});
    let req = test::TestRequest::post()
        .uri("/api/v1/admin/articles")
        .cookie(session.clone())
        .set_json(&payload)
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::FORBIDDEN
    );
    let req = test::TestRequest::post()
        .uri("/api/v1/admin/articles")
        .cookie(session.clone())
        .insert_header(("X-CSRF-Token", csrf.clone()))
        .set_json(&payload)
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let req = test::TestRequest::get()
        .uri("/api/v1/blogs/test-article")
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
    let req = test::TestRequest::post()
        .uri("/api/v1/admin/articles/test-article/publish")
        .set_json(json!({"expected_revision":1}))
        .cookie(session.clone())
        .insert_header(("X-CSRF-Token", csrf.clone()))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
    let req = test::TestRequest::get()
        .uri("/api/v1/blogs/test-article")
        .to_request();
    let response = test::call_service(&app, req).await;
    let etag = response.headers().get(header::ETAG).unwrap().clone();
    assert!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .unwrap()
            .to_str()
            .unwrap()
            .contains("stale-while-revalidate")
    );
    let body: Value = test::read_body_json(response).await;
    assert!(body.get("document").is_none());
    assert_eq!(body["compiled"]["format"], "publisle:publication");
    let req = test::TestRequest::get()
        .uri("/api/v1/blogs/test-article")
        .insert_header((header::IF_NONE_MATCH, etag))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_MODIFIED
    );
    let req = test::TestRequest::get()
        .uri("/api/v1/blogs?page=9223372036854775807")
        .to_request();
    let body: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["total"], 1);
    assert!(body["items"].as_array().unwrap().is_empty());
    let req = test::TestRequest::get()
        .uri("/sitemap-blogs.xml")
        .to_request();
    let xml = test::call_and_read_body(&app, req).await;
    let xml = std::str::from_utf8(&xml).unwrap();
    assert!(xml.contains("/blog/test-article"));
    assert!(!xml.contains("how-to-validate"));
    // Published edits must include compilation; a failed update keeps the current article.
    let req = test::TestRequest::put()
        .uri("/api/v1/admin/articles/test-article")
        .cookie(session.clone())
        .insert_header(("X-CSRF-Token", csrf.clone()))
        .set_json(json!({"document":document("Edited"),"expected_revision":2}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let req = test::TestRequest::post()
        .uri("/api/v1/blogs/test-article/comments")
        .set_json(json!({"name":"Reader","body":"<script>alert(1)</script>"}))
        .to_request();
    let body: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["status"], "pending");
    let comment_id = body["id"].as_str().unwrap();
    let req = test::TestRequest::get()
        .uri("/api/v1/blogs/test-article/comments")
        .to_request();
    let body: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["total"], 0);
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/admin/comments/{comment_id}/approve"))
        .cookie(session.clone())
        .insert_header(("X-CSRF-Token", csrf.clone()))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
    let req = test::TestRequest::get()
        .uri("/api/v1/blogs/test-article/comments")
        .to_request();
    let body: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["body"], "<script>alert(1)</script>");
    assert!(body["items"][0].get("ip_hash").is_none());
    let req = test::TestRequest::post()
        .uri("/api/v1/admin/articles/test-article/unpublish")
        .set_json(json!({"expected_revision":2}))
        .cookie(session.clone())
        .insert_header(("X-CSRF-Token", csrf.clone()))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
    let req = test::TestRequest::put()
        .uri("/api/v1/admin/articles/test-article")
        .cookie(session.clone())
        .insert_header(("X-CSRF-Token", csrf.clone()))
        .set_json(json!({"document":document("Edited"),"expected_revision":3}))
        .to_request();
    let body: Value = test::call_and_read_body_json(&app, req).await;
    assert!(body.get("compiled").is_none());
    let req = test::TestRequest::post()
        .uri("/api/v1/admin/articles/test-article/publish")
        .set_json(json!({"expected_revision":4}))
        .cookie(session.clone())
        .insert_header(("X-CSRF-Token", csrf.clone()))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let req = test::TestRequest::put()
        .uri("/api/v1/admin/articles/how-to-validate-a-semantic-color-system")
        .cookie(session.clone())
        .insert_header(("X-CSRF-Token", csrf.clone()))
        .set_json(json!({"featured":true,"expected_revision":1}))
        .to_request();
    let body: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["document"]["schemaVersion"], 1);
    assert!(!body["document"]["blocks"].as_array().unwrap().is_empty());
    let req = test::TestRequest::delete()
        .uri("/api/v1/admin/session")
        .cookie(session.clone())
        .insert_header(("X-CSRF-Token", csrf))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
    let req = test::TestRequest::get()
        .uri("/api/v1/admin/session")
        .cookie(session)
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[actix_web::test]
async fn oauth_start_requires_pkce_and_provider_bound_state() {
    let mut config = Config::for_test();
    config.google_client_id = Some("google-client".into());
    config.google_client_secret = Some("secret".into());
    config.github_client_id = Some("github-client".into());
    config.github_client_secret = Some("secret".into());
    let app = test::init_service(build(database().await, config)).await;
    let request = test::TestRequest::get()
        .uri("/api/v1/auth/providers")
        .to_request();
    let available: Value = test::call_and_read_body_json(&app, request).await;
    assert_eq!(available, json!(["google", "github"]));
    for provider in ["google", "github"] {
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/auth/{provider}/start"))
            .to_request();
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::FOUND);
        let location = response
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(location.contains("code_challenge_method=S256"));
        let cookies: Vec<_> = response.response().cookies().collect();
        assert!(cookies.iter().any(|c| c.name() == "voidreg_oauth_verifier"));
        let state = cookies
            .iter()
            .find(|c| c.name() == "voidreg_oauth_state")
            .unwrap();
        assert!(state.value().starts_with(provider));
        let req = test::TestRequest::get()
            .uri(&format!(
                "/api/v1/auth/{provider}/callback?code=fake&state=wrong"
            ))
            .cookie(state.clone())
            .to_request();
        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
}
