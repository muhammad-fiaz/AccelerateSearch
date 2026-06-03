//! End-to-end tests for the REST API surface.
//!
//! These tests use `actix_web::test` to spin up an isolated app and
//! drive the HTTP endpoints without touching a real socket.

#[cfg(test)]
mod openapi_tests {
    use serde_json::Value;
    use utoipa::OpenApi;

    use crate::openapi::ApiDoc;

    #[test]
    fn contains_all_snapshot_routes() {
        let json: Value = serde_json::to_value(ApiDoc::openapi()).expect("serialize openapi");
        let paths = json
            .get("paths")
            .and_then(|p| p.as_object())
            .expect("paths object");

        assert!(
            paths.contains_key("/api/v1/snapshots"),
            "POST/GET /api/v1/snapshots missing"
        );
        assert!(
            paths.contains_key("/api/v1/snapshots/{name}"),
            "GET/DELETE /api/v1/snapshots/{{name}} missing"
        );
        assert!(
            paths.contains_key("/api/v1/snapshots/{name}/restore"),
            "POST /api/v1/snapshots/{{name}}/restore missing"
        );

        let post = &paths["/api/v1/snapshots"];
        assert!(post.get("post").is_some(), "POST on /api/v1/snapshots");
        assert!(post.get("get").is_some(), "GET on /api/v1/snapshots");

        let item = &paths["/api/v1/snapshots/{name}"];
        assert!(
            item.get("get").is_some(),
            "GET on /api/v1/snapshots/{{name}}"
        );
        assert!(
            item.get("delete").is_some(),
            "DELETE on /api/v1/snapshots/{{name}}"
        );

        let restore = &paths["/api/v1/snapshots/{name}/restore"];
        assert!(restore.get("post").is_some(), "POST restore endpoint");
    }
}

#[cfg(test)]
mod routing_tests {
    use actix_web::{App, http::StatusCode, test};

    use crate::configure_routes;

    #[actix_web::test]
    async fn snapshot_routes_return_response_for_missing_state() {
        let app = test::init_service(App::new().configure(configure_routes)).await;
        let req = test::TestRequest::get()
            .uri("/api/v1/snapshots/does-not-exist")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(matches!(
            resp.status(),
            StatusCode::NOT_FOUND | StatusCode::INTERNAL_SERVER_ERROR
        ));
    }
}
