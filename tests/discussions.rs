use actix_web::{cookie::Cookie, http::StatusCode, test};
use base64::Engine as _;
use serde_json::{Value, json};
use voidreg_api::{
    app::build,
    config::Config,
    db::{ArticleFields, Db},
};

async fn database() -> Db {
    let db = Db::memory().await.unwrap();
    db.migrate().await.unwrap();
    db.migrate().await.unwrap();
    for slug in ["discussion", "other"] {
        db.insert_article(ArticleFields {
            revision: 1,
            slug: slug.into(),
            title: slug.into(),
            category: "Systems".into(),
            summary: "Summary".into(),
            status: "published".into(),
            featured: false,
            sort_order: 0,
            published_at: Some("2026-10-15T00:00:00Z".into()),
            reading_time_minutes: None,
            sections: "[]".into(),
            document: String::new(),
            document_version: 0,
            compiled: String::new(),
            content_hash: String::new(),
        })
        .await
        .unwrap();
    }
    db
}

#[actix_web::test]
async fn discussion_replies_pagination_moderation_and_removal() {
    let db = database().await;
    let identity = db
        .upsert_identity(
            "github",
            "test-reader",
            "reader@example.com",
            "Verified Reader",
            None,
        )
        .await
        .unwrap();
    let (reader, hash) = voidreg_api::auth::new_session_token();
    db.create_session("commenter", &hash, Some(&identity.id), 1)
        .await
        .unwrap();
    let reader = Cookie::new("voidreg_commenter_session", reader);
    let (admin, hash) = voidreg_api::auth::new_session_token();
    db.create_session("admin", &hash, None, 1).await.unwrap();
    let csrf = voidreg_api::api::v1::admin::session::csrf_for(&admin);
    let admin = Cookie::new("voidreg_admin_session", admin);
    let app = test::init_service(build(db.clone(), Config::for_test())).await;
    let body: Value = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/blogs/discussion/comments")
            .cookie(reader.clone())
            .set_json(json!({"name":"Spoofed","body":"Root question"}))
            .to_request(),
    )
    .await;
    let root = body["id"].as_str().unwrap().to_owned();
    assert_eq!(body["status"], "approved");
    let body: Value = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/blogs/discussion/comments")
            .set_json(
                json!({"name":"Alice","body":"<script>literal reply</script>","parent_id":root}),
            )
            .to_request(),
    )
    .await;
    let reply = body["id"].as_str().unwrap().to_owned();
    assert_eq!(body["status"], "pending");
    let list: Value = test::call_and_read_body_json(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/blogs/discussion/comments")
            .to_request(),
    )
    .await;
    assert_eq!(list["total"], 1);
    assert_eq!(list["items"][0]["reply_count"], 0);
    assert_eq!(list["items"][0]["author_name"], "Verified Reader");
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/blogs/discussion/comments")
            .cookie(reader.clone())
            .set_json(json!({"name":"Ignored","body":"Reply to pending","parent_id":reply}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let moderation: Value = test::call_and_read_body_json(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/admin/comments?status=pending")
            .cookie(admin.clone())
            .to_request(),
    )
    .await;
    assert_eq!(moderation["items"][0]["parent_author"], "Verified Reader");
    assert_eq!(moderation["items"][0]["parent_body"], "Root question");
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/admin/comments/{reply}/approve"))
            .cookie(admin.clone())
            .insert_header(("X-CSRF-Token", csrf.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let nested: Value = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/blogs/discussion/comments")
            .cookie(reader.clone())
            .set_json(json!({"name":"Ignored","body":"Nested response","parent_id":reply}))
            .to_request(),
    )
    .await;
    let nested = nested["id"].as_str().unwrap();
    let item = db.get_comment(nested).await.unwrap().unwrap();
    assert_eq!(item.parent_id.as_deref(), Some(reply.as_str()));
    assert_eq!(item.thread_id.as_deref(), Some(root.as_str()));
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/blogs/other/comments")
            .cookie(reader.clone())
            .set_json(json!({"name":"Ignored","body":"Cross article","parent_id":root}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/blogs/discussion/comments")
            .cookie(reader.clone())
            .set_json(json!({"name":"Ignored","body":"Missing target","parent_id":"missing"}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let _: Value = test::call_and_read_body_json(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/blogs/discussion/comments")
            .cookie(reader.clone())
            .set_json(json!({"name":"Ignored","body":"Another root"}))
            .to_request(),
    )
    .await;
    let list: Value = test::call_and_read_body_json(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/blogs/discussion/comments?per_page=1")
            .to_request(),
    )
    .await;
    assert_eq!(list["total"], 2);
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    let replies: Value = test::call_and_read_body_json(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/api/v1/blogs/discussion/comments/{root}/replies?per_page=1&page=2"
            ))
            .to_request(),
    )
    .await;
    assert_eq!(replies["total"], 2);
    assert_eq!(replies["items"][0]["id"], nested);
    assert_eq!(replies["items"][0]["replying_to"], "Alice");
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/blogs/other/comments/{root}/replies"))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    for id in [&root, &reply] {
        let response = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v1/admin/comments/{id}"))
                .cookie(admin.clone())
                .insert_header(("X-CSRF-Token", csrf.clone()))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    let list: Value = test::call_and_read_body_json(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/blogs/discussion/comments")
            .to_request(),
    )
    .await;
    let root_item = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == root)
        .unwrap();
    assert_eq!(root_item["removed"], true);
    assert_eq!(root_item["body"], "");
    assert_eq!(root_item["author_name"], "Removed comment");
    assert_eq!(root_item["reply_count"], 2);
    let replies: Value = test::call_and_read_body_json(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/blogs/discussion/comments/{root}/replies"))
            .to_request(),
    )
    .await;
    assert_eq!(replies["items"][0]["removed"], true);
    assert_eq!(replies["items"][1]["body"], "Nested response");
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/blogs/discussion/comments")
            .cookie(reader)
            .set_json(json!({"name":"Ignored","body":"Removed target","parent_id":root}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/admin/comments/{root}/approve"))
            .cookie(admin)
            .insert_header(("X-CSRF-Token", csrf))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[actix_web::test]
async fn oauth_return_is_limited_to_local_articles_and_state_bound() {
    let mut config = Config::for_test();
    config.github_client_id = Some("client".into());
    config.github_client_secret = Some("secret".into());
    let app = test::init_service(build(database().await, config)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/auth/github/start?return_to=%2Fblog%2Fdiscussion%23discussion")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FOUND);
    let state = response
        .response()
        .cookies()
        .find(|c| c.name() == "voidreg_oauth_state")
        .unwrap()
        .into_owned();
    let path = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(state.value().split(':').nth(3).unwrap())
        .unwrap();
    assert_eq!(
        String::from_utf8(path).unwrap(),
        "/blog/discussion#discussion"
    );
    for target in [
        "https%3A%2F%2Fevil.example",
        "%2F%2Fevil.example",
        "%2Fblog%2F..%2Fadmin",
        "%2Fblog%2F%5Cevil",
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v1/auth/github/start?return_to={target}"))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
