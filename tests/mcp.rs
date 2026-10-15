use actix_web::{cookie::Cookie, http::StatusCode, test};
use serde_json::{Value, json};
use voidreg_api::{
    app::build,
    auth::{hash_token, new_session_token},
    config::Config,
    db::Db,
};

#[actix_web::test]
async fn tokens_draft_permissions_revisions_revocation_and_owner_isolation() {
    let db = Db::memory().await.unwrap();
    db.migrate().await.unwrap();
    db.migrate().await.unwrap();
    let mut config = Config::for_test();
    config.admin_github_ids = vec!["owner".into(), "second".into()];
    let owner = db
        .upsert_identity("github", "owner", "", "Owner", None)
        .await
        .unwrap();
    let second = db
        .upsert_identity("github", "second", "", "Second", None)
        .await
        .unwrap();
    let (admin, hash) = new_session_token();
    db.create_session("admin", &hash, Some(&owner.id), 1)
        .await
        .unwrap();
    let csrf = voidreg_api::api::v1::admin::session::csrf_for(&admin);
    let cookie = Cookie::new("voidreg_admin_session", admin);
    let app = test::init_service(build(db.clone(), config.clone())).await;
    let request = test::TestRequest::post()
        .uri("/api/v1/admin/mcp-tokens")
        .cookie(cookie.clone())
        .set_json(json!({"name":"CLI"}))
        .to_request();
    assert_eq!(
        test::call_service(&app, request).await.status(),
        StatusCode::FORBIDDEN
    );
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/admin/mcp-tokens")
            .cookie(cookie.clone())
            .insert_header(("X-CSRF-Token", csrf.clone()))
            .set_json(json!({"name":"CLI"}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let created: Value = test::read_body_json(response).await;
    let token = created["token"].as_str().unwrap();
    let id = created["connection"]["id"].as_str().unwrap();
    let auth = format!("Bearer {token}");
    assert!(created["connection"].get("token_hash").is_none());
    assert!(db.list_mcp_tokens(&second.id).await.unwrap().is_empty());
    db.revoke_mcp_token(id, &second.id).await.unwrap();
    assert!(
        db.mcp_token_identity(&hash_token(token))
            .await
            .unwrap()
            .is_some()
    );
    for header in [None, Some("Bearer bad")] {
        let mut request = test::TestRequest::get().uri("/api/v1/mcp/session");
        if let Some(header) = header {
            request = request.insert_header(("Authorization", header));
        }
        assert_eq!(
            test::call_service(&app, request.to_request())
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let expired = "vr_mcp_expired";
    db.create_mcp_token(&owner.id, "Expired", &hash_token(expired), -1)
        .await
        .unwrap();
    assert_eq!(
        test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/mcp/session")
                .insert_header(("Authorization", format!("Bearer {expired}")))
                .to_request()
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/admin/articles")
                .insert_header(("Authorization", auth.clone()))
                .to_request()
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let document = json!({"schemaVersion":1,"blocks":[]});
    let compiled = json!({"format":"publisle:publication","formatVersion":1,"identity":"test","rendererBuild":"1","html":"","styles":[],"modules":[],"islands":[],"diagnostics":[]});
    let body: Value = test::call_and_read_body_json(&app, test::TestRequest::post().uri("/api/v1/mcp/articles").insert_header(("Authorization", auth.clone())).set_json(json!({"slug":"mcp-draft","title":"Draft","category":"Systems","summary":"Summary","document":document,"compiled":compiled})).to_request()).await;
    assert_eq!(body["revision"], 1);
    assert_eq!(body["status"], "draft");
    let update = json!({"title":"Updated","expected_revision":1});
    let request = || {
        test::TestRequest::put()
            .uri("/api/v1/mcp/articles/mcp-draft")
            .insert_header(("Authorization", auth.clone()))
            .set_json(&update)
            .to_request()
    };
    let (left, right) = tokio::join!(
        test::call_service(&app, request()),
        test::call_service(&app, request())
    );
    assert!(matches!(
        (left.status(), right.status()),
        (StatusCode::OK, StatusCode::CONFLICT) | (StatusCode::CONFLICT, StatusCode::OK)
    ));
    let publish = |revision| {
        test::TestRequest::post()
            .uri("/api/v1/admin/articles/mcp-draft/publish")
            .cookie(cookie.clone())
            .insert_header(("X-CSRF-Token", csrf.clone()))
            .set_json(json!({"expected_revision":revision}))
            .to_request()
    };
    assert_eq!(
        test::call_service(&app, publish(1)).await.status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        test::call_service(&app, publish(2)).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/api/v1/mcp/articles/mcp-draft")
                .insert_header(("Authorization", auth.clone()))
                .set_json(json!({"title":"Live edit","expected_revision":3}))
                .to_request()
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/mcp/articles/mcp-draft/publish")
                .insert_header(("Authorization", auth.clone()))
                .to_request()
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let current = db.admin_get_article("mcp-draft").await.unwrap();
    assert_eq!(current.title, "Updated");
    assert_eq!(current.revision, 3);
    // Losing the allowlist invalidates existing tokens without waiting for expiry.
    config.admin_github_ids.clear();
    let denied = test::init_service(build(db.clone(), config)).await;
    assert_eq!(
        test::call_service(
            &denied,
            test::TestRequest::get()
                .uri("/api/v1/mcp/session")
                .insert_header(("Authorization", auth.clone()))
                .to_request()
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let tokens: Value = test::call_and_read_body_json(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/admin/mcp-tokens")
            .cookie(cookie.clone())
            .to_request(),
    )
    .await;
    assert!(
        tokens["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item.get("token_hash").is_none() && item.get("token").is_none())
    );
    assert!(
        db.list_mcp_tokens(&owner.id)
            .await
            .unwrap()
            .iter()
            .any(|t| t.last_used_at.is_some())
    );
    assert_eq!(
        test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v1/admin/mcp-tokens/{id}"))
                .cookie(cookie)
                .insert_header(("X-CSRF-Token", csrf))
                .to_request()
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/mcp/session")
                .insert_header(("Authorization", auth))
                .to_request()
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[actix_web::test]
async fn conditional_write_prevents_publish_races_and_requires_draft_status() {
    use voidreg_api::db::ArticleFields;
    let db = Db::memory().await.unwrap();
    db.migrate().await.unwrap();
    let draft = ArticleFields {
        revision: 1,
        slug: "racing-draft".into(),
        title: "Original".into(),
        category: "Systems".into(),
        summary: "Summary".into(),
        status: "draft".into(),
        featured: false,
        sort_order: 0,
        published_at: None,
        reading_time_minutes: None,
        sections: "[]".into(),
        document: String::new(),
        document_version: 0,
        compiled: String::new(),
        content_hash: String::new(),
    };
    db.insert_article(draft.clone()).await.unwrap();
    let mut published = draft.clone();
    published.status = "published".into();
    let mut edited = draft.clone();
    edited.title = "MCP edit".into();
    let (publish, edit) = tokio::join!(
        db.save_article(&published, false),
        db.save_article(&edited, true)
    );
    assert_ne!(publish.is_ok(), edit.is_ok());
    let current = db.admin_get_article(&draft.slug).await.unwrap();
    assert_eq!(current.revision, 2);
    if publish.is_ok() {
        assert_eq!(current.status.as_str(), "published");
        assert_eq!(current.title, "Original");
        // Even a current revision cannot turn a live article back into a draft.
        edited.revision = current.revision;
        assert!(matches!(
            db.save_article(&edited, true).await,
            Err(voidreg_api::error::AppError::Conflict(_))
        ));
    } else {
        assert_eq!(current.status.as_str(), "draft");
        assert_eq!(current.title, "MCP edit");
    }
}
