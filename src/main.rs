mod post_service;

use axum::{
    extract::Extension,
    routing::{delete, get, patch, post},
    Router, Server,
};

use migration::{Migrator, MigratorTrait};
use post_service::*;

use sea_orm::Database;

use std::str::FromStr;
use std::{env, net::SocketAddr};
use tokio::signal;
use tower::ServiceBuilder;
// Quick instructions
//
// - get an authorization token:
//
// curl -s \
//     -w '\n' \
//     -H 'Content-Type: application/json' \
//     -d '{"client_id":"foo","client_secret":"bar"}' \
//     http://localhost:8000/authorize
//
// - visit the protected area using the authorized token
//
// curl -s \
//     -w '\n' \
//     -H 'Content-Type: application/json' \
//     -H 'Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJiQGIuY29tIiwiY29tcGFueSI6IkFDTUUiLCJleHAiOjIwMDAwMDAwMDB9.ULPZ0NLBq9tfHroRgxJJeEYCy0tguZrEwix3fo-2dFc' \
//      http://localhost:8000/api/\?page\=1\&posts_per_page\=100
//
// - try to visit the protected area using an invalid token
//
// curl -s \
//     -w '\n' \
//     -H 'Content-Type: application/json' \
//     -H 'Authorization: Bearer blahblahblah' \
//     http://localhost:8000/protected

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

    let addr = SocketAddr::from_str(&server_url).unwrap();
    let app = app().layer(ServiceBuilder::new().layer(Extension(conn)));
    Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    Ok(())
}
fn app() -> Router {
    Router::new()
        .route("/hello/", get(|| async { "Hello, World!" }))
        .route("/api/", get(api_list_posts))
        .route("/api/", post(api_create_post))
        .route("/api/:id", patch(api_update_post))
        .route("/api/:id", delete(api_delete_post))
        .route("/authorize", post(login))
}
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("signal received, starting graceful shutdown");
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
