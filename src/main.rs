mod post_service;

use axum::{
    extract::Extension,
    http::StatusCode,
    routing::{delete, get, get_service, patch, post},
    Router, Server,
};

use migration::{Migrator, MigratorTrait};
use post_service::*;

use sea_orm::Database;

use std::str::FromStr;
use std::{env, net::SocketAddr};
use tera::Tera;
use tower::ServiceBuilder;
use tower_cookies::CookieManagerLayer;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env::set_var("RUST_LOG", "debug");
    tracing_subscriber::fmt::init();

    dotenv::dotenv().ok();
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL is not set in .env file");
    let host = env::var("HOST").expect("HOST is not set in .env file");
    let port = env::var("PORT").expect("PORT is not set in .env file");
    let server_url = format!("{}:{}", host, port);

    let conn = Database::connect(db_url)
        .await
        .expect("Database connection failed");
    Migrator::up(&conn, None).await.unwrap();
    let templates = Tera::new(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/**/*"))
        .expect("Tera initialization failed");
    // let state = AppState { templates, conn };

    let addr = SocketAddr::from_str(&server_url).unwrap();
    let app = app().layer(
        ServiceBuilder::new()
            .layer(CookieManagerLayer::new())
            .layer(Extension(conn))
            .layer(Extension(templates)),
    );
    Server::bind(&addr).serve(app.into_make_service()).await?;

    Ok(())
}
fn app() -> Router {
    Router::new()
        .route("/hello/", get(|| async { "Hello, World!" }))
        .route("/api/", get(api_list_posts).post(api_create_post))
        .route("/api/:id", patch(api_update_post))
        .route("/api/:id", delete(api_delete_post))
        .route("/", get(list_posts).post(create_post))
        .route("/:id", get(edit_post).post(update_post))
        .route("/new", get(new_post))
        .route("/delete/:id", post(delete_post))
        .nest(
            "/static",
            get_service(ServeDir::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/static"
            )))
            .handle_error(|error: std::io::Error| async move {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unhandled internal error: {}", error),
                )
            }),
        )
}
#[cfg(test)]
mod tests {
    use super::*;

    use axum::{
        body::Body,
        http::{self, Request, StatusCode},
    };
    use serde_json::{json, Value};
    use tower::ServiceExt; // for `oneshot` and `ready`

    async fn mock_app() -> Router {
        let conn = Database::connect("sqlite::memory:".to_string())
            .await
            .expect("Database connection failed");
        Migrator::up(&conn, None).await.unwrap();
        app().layer(ServiceBuilder::new().layer(Extension(conn)))
    }

    #[tokio::test]
    async fn hello_world() {
        let app = mock_app().await;

        // `Router` implements `tower::Service<Request<Body>>` so we can
        // call it like any tower service, no need to run an HTTP server.
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/hello/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(&body, "Hello, World!");
        assert_eq!(&body[..], b"Hello, World!");
    }

    // #[tokio::test]
    // async fn multiple_request() {
    //     let mut app = app();

    //     let request = Request::builder()
    //         .uri("/hello/")
    //         .body(Body::empty())
    //         .unwrap();
    //     let response = app.ready().await.unwrap().call(request).await.unwrap();
    //     assert_eq!(response.status(), StatusCode::OK);

    //     let request = Request::builder()
    //         .uri("/hello/")
    //         .body(Body::empty())
    //         .unwrap();
    //     let response = app.ready().await.unwrap().call(request).await.unwrap();
    //     assert_eq!(response.status(), StatusCode::OK);
    // }
    #[tokio::test]
    async fn json() {
        let app = mock_app().await;
        // - list
        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::GET)
                    .uri("/api/")
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            body,
            json!({"num_pages": 0, "page": 1, "posts": [], "posts_per_page": 5})
        );
        // // - new
        // let response = app
        //     .oneshot(
        //         Request::builder()
        //             .method(http::Method::POST)
        //             .uri("/api/")
        //             .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
        //             .body(Body::from(
        //                 serde_json::to_vec(
        //                     &json!({"title": "title11", "text":"text11","new_col":17}),
        //                 )
        //                 .unwrap(),
        //             ))
        //             .unwrap(),
        //     )
        //     .await
        //     .unwrap();

        // assert_eq!(response.status(), StatusCode::OK);

        // let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        // let body: Value = serde_json::from_slice(&body).unwrap();
        // assert_eq!(
        //     body,
        //     json!({
        //         "kind": "success",
        //         "message": "Post succcessfully added",
        //     })
        // );
        // // - list
        // let response = app
        //     .oneshot(
        //         Request::builder()
        //             .method(http::Method::GET)
        //             .uri("/api/")
        //             .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
        //             .body(Body::empty())
        //             .unwrap(),
        //     )
        //     .await
        //     .unwrap();

        // assert_eq!(response.status(), StatusCode::OK);

        // let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        // let body: Value = serde_json::from_slice(&body).unwrap();
        // assert_eq!(
        //     body,
        //     json!({"num_pages": 0, "page": 1, "posts": [], "posts_per_page": 5})
        // );

        // - update
        // - delete
        // - list
    }
}
