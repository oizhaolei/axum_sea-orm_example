use lazy_static::lazy_static;
use ring::hmac;
use ring::hmac::Key;
use std::fmt::Display;

use axum::{
    async_trait,
    extract::{Extension, FromRequest, Path, Query, RequestParts, TypedHeader},
    headers::{authorization::Bearer, Authorization},
    response::{IntoResponse, Response},
    Json,
};
use entity::posts::{self, Model};
use serde_json::json;

use entity::user;
use hyper::StatusCode;
use posts::Entity as Posts;
use sea_orm::{prelude::*, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use user::Entity as User;

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};

lazy_static! {
    static ref SECRET: String = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    static ref KEY: Key = hmac::Key::new(hmac::HMAC_SHA256, SECRET.as_bytes());
    static ref KEYS: Keys = Keys::new(SECRET.as_bytes());
}

#[derive(Deserialize)]
pub struct Params {
    page: Option<usize>,
    posts_per_page: Option<usize>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PaginationPost {
    posts: Vec<Model>,
    page: usize,
    posts_per_page: usize,
    num_pages: usize,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct FlashData {
    kind: String,
    message: String,
}

// curl http://localhost:8000/api/?page\=1&posts_per_page=100
pub async fn api_list_posts(
    claims: Claims,
    Extension(ref conn): Extension<DatabaseConnection>,
    Query(params): Query<Params>,
) -> impl IntoResponse {
    tracing::info!("claims: {:?}", claims);
    let page = params.page.unwrap_or(1);
    let posts_per_page = params.posts_per_page.unwrap_or(5);
    let paginator = Posts::find()
        .order_by_asc(posts::Column::Id)
        .paginate(conn, posts_per_page);
    let num_pages = paginator.num_pages().await.ok().unwrap();
    let posts = paginator
        .fetch_page(page - 1)
        .await
        .expect("could not retrieve posts");

    let page = PaginationPost {
        posts,
        page,
        posts_per_page,
        num_pages,
    };

    Json(page)
}

// curl -X POST -H 'Content-Type: application/json' http://localhost:8000/api/ --data '{"title": "title11", "text":"text11","new_col":0}'
pub async fn api_create_post(
    claims: Claims,
    Extension(ref conn): Extension<DatabaseConnection>,
    Json(input): Json<posts::Model>,
) -> impl IntoResponse {
    tracing::info!("claims: {:?}", claims);
    posts::ActiveModel {
        title: Set(input.title.to_owned()),
        text: Set(input.text.to_owned()),
        new_col: Set(input.new_col.to_owned()),
        ..Default::default()
    }
    .save(conn)
    .await
    .expect("could not insert post");

    let data = FlashData {
        kind: "success".to_owned(),
        message: "Post succcessfully added".to_owned(),
    };

    Json(data)
}

// curl -X PATCH -H 'Content-Type: application/json' http://localhost:8000/api/12 --data '{"title": "title11", "text":"text11","new_col":4}'
pub async fn api_update_post(
    claims: Claims,
    Extension(ref conn): Extension<DatabaseConnection>,
    Path(id): Path<i32>,
    Json(input): Json<posts::Model>,
) -> impl IntoResponse {
    tracing::info!("claims: {:?}", claims);
    posts::ActiveModel {
        id: Set(id),
        title: Set(input.title.to_owned()),
        text: Set(input.text.to_owned()),
        new_col: Set(input.new_col.to_owned()),
    }
    .save(conn)
    .await
    .expect("could not edit post");

    let data = FlashData {
        kind: "success".to_owned(),
        message: "Post succcessfully updated".to_owned(),
    };

    Json(data)
}

// curl -X DELETE  http://localhost:8000/api/12
pub async fn api_delete_post(
    claims: Claims,
    Extension(ref conn): Extension<DatabaseConnection>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    tracing::info!("claims: {:?}", claims);
    let post: posts::ActiveModel = Posts::find_by_id(id)
        .one(conn)
        .await
        .unwrap()
        .unwrap()
        .into();

    post.delete(conn).await.unwrap();

    let data = FlashData {
        kind: "success".to_owned(),
        message: "Post succcessfully deleted".to_owned(),
    };

    Json(data)
}
#[cfg(test)]
mod tests {

    use super::*;

    use migration::{Migrator, MigratorTrait};
    use sea_orm::Database;
    use serde_json::json;
    #[tokio::test]
    async fn hello_world() {
        let conn = Database::connect("sqlite::memory:".to_string())
            .await
            .expect("Database connection failed");
        Migrator::up(&conn, None).await.unwrap();

        //list
        let page = 1;
        let posts_per_page = 5;
        let paginator = Posts::find()
            .order_by_asc(posts::Column::Id)
            .paginate(&conn, posts_per_page);
        let posts = paginator
            .fetch_page(page - 1)
            .await
            .expect("could not retrieve posts");
        assert_eq!(0, posts.len());
        assert_eq!(json!(posts), json!([]));

        // create
        posts::ActiveModel {
            title: Set("title11".to_owned()),
            text: Set("text11".to_owned()),
            new_col: Set(17),
            ..Default::default()
        }
        .save(&conn)
        .await
        .expect("could not insert post");

        //list
        let page = 1;
        let posts_per_page = 5;
        let paginator = Posts::find()
            .order_by_asc(posts::Column::Id)
            .paginate(&conn, posts_per_page);
        let posts = paginator
            .fetch_page(page - 1)
            .await
            .expect("could not retrieve posts");
        assert_eq!(1, posts.len());
        assert_eq!(posts[0].title, "title11");
        assert_eq!(posts[0].text, "text11");
        assert_eq!(posts[0].new_col, 17);
    }
}

pub async fn authorize_user(
    Json(payload): Json<AuthPayload>,
    Extension(ref conn): Extension<DatabaseConnection>,
) -> Result<Json<AuthBody>, AuthError> {
    // Check if the user sent the credentials
    if payload.client_id.is_empty() || payload.client_secret.is_empty() {
        return Err(AuthError::MissingCredentials);
    }
    // Here you can check the user credentials from a database
    let user: user::Model = User::find()
        .filter(user::Column::Email.eq(payload.client_id))
        .one(conn)
        .await
        .expect("could not find user")
        .unwrap();
    let tag = hmac::sign(&KEY, payload.client_secret.as_bytes());
    let client_secret_hash = base64::encode(tag.as_ref());
    tracing::info!(
        "user.hash: {:?}, client_secret_hash: {:?}",
        user.hash,
        client_secret_hash
    );
    if user.hash != client_secret_hash {
        return Err(AuthError::WrongCredentials);
    }
    let claims = Claims {
        sub: "b@b.com".to_owned(),
        company: "ACME".to_owned(),
        // Mandatory expiry time as UTC timestamp
        exp: 2000000000, // May 2033
    };
    // Create the authorization token
    let token = encode(&Header::default(), &claims, &KEYS.encoding)
        .map_err(|_| AuthError::TokenCreation)?;

    // Send the authorized token
    Ok(Json(AuthBody::new(token)))
}

impl Display for Claims {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Email: {}\nCompany: {}", self.sub, self.company)
    }
}

impl AuthBody {
    fn new(access_token: String) -> Self {
        Self {
            access_token,
            token_type: "Bearer".to_string(),
        }
    }
}

#[async_trait]
impl<S> FromRequest<S> for Claims
where
    S: Send,
{
    type Rejection = AuthError;

    async fn from_request(req: &mut RequestParts<S>) -> Result<Self, Self::Rejection> {
        // Extract the token from the authorization header
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request(req)
                .await
                .map_err(|_| AuthError::InvalidToken)?;
        // Decode the user data
        let token_data = decode::<Claims>(bearer.token(), &KEYS.decoding, &Validation::default())
            .map_err(|_| AuthError::InvalidToken)?;

        Ok(token_data.claims)
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AuthError::WrongCredentials => (StatusCode::UNAUTHORIZED, "Wrong credentials"),
            AuthError::MissingCredentials => (StatusCode::BAD_REQUEST, "Missing credentials"),
            AuthError::TokenCreation => (StatusCode::INTERNAL_SERVER_ERROR, "Token creation error"),
            AuthError::InvalidToken => (StatusCode::BAD_REQUEST, "Invalid token"),
        };
        let body = Json(json!({
            "error": error_message,
        }));
        (status, body).into_response()
    }
}

struct Keys {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl Keys {
    fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    sub: String,
    company: String,
    exp: usize,
}

#[derive(Debug, Serialize)]
pub struct AuthBody {
    access_token: String,
    token_type: String,
}

#[derive(Debug, Deserialize)]
pub struct AuthPayload {
    client_id: String,
    client_secret: String,
}

#[derive(Debug)]
pub enum AuthError {
    WrongCredentials,
    MissingCredentials,
    TokenCreation,
    InvalidToken,
}
