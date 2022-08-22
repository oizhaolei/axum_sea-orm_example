use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::{
    extract::{Extension, Form, Path, Query},
    response::Html,
    response::IntoResponse,
    Json,
};
use entity::posts::{self, Model};

use posts::Entity as Posts;
use sea_orm::{prelude::*, QueryOrder, Set};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use tera::Tera;
use tower_cookies::{Cookie, Cookies};

#[derive(Deserialize)]
struct ValuedMessage<T> {
    #[serde(rename = "_")]
    value: T,
}

#[derive(Serialize)]
struct ValuedMessageRef<'a, T> {
    #[serde(rename = "_")]
    value: &'a T,
}

const FLASH_COOKIE_NAME: &str = "_flash";

pub fn get_flash_cookie<T>(cookies: &Cookies) -> Option<T>
where
    T: DeserializeOwned,
{
    cookies.get(FLASH_COOKIE_NAME).and_then(|flash_cookie| {
        if let Ok(ValuedMessage::<T> { value }) = serde_json::from_str(flash_cookie.value()) {
            Some(value)
        } else {
            None
        }
    })
}

pub type PostResponse = (StatusCode, HeaderMap);

pub fn post_response<T>(cookies: &mut Cookies, data: T) -> PostResponse
where
    T: Serialize,
{
    let valued_message_ref = ValuedMessageRef { value: &data };

    let mut cookie = Cookie::new(
        FLASH_COOKIE_NAME,
        serde_json::to_string(&valued_message_ref).unwrap(),
    );
    cookie.set_path("/");
    cookies.add(cookie);

    let mut header = HeaderMap::new();
    header.insert(header::LOCATION, HeaderValue::from_static("/"));

    (StatusCode::SEE_OTHER, header)
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
    Extension(ref conn): Extension<DatabaseConnection>,
    Query(params): Query<Params>,
) -> impl IntoResponse {
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
pub async fn list_posts(
    Extension(ref templates): Extension<Tera>,
    Extension(ref conn): Extension<DatabaseConnection>,
    Query(params): Query<Params>,
    cookies: Cookies,
) -> Result<Html<String>, (StatusCode, &'static str)> {
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

    let mut ctx = tera::Context::new();
    ctx.insert("posts", &posts);
    ctx.insert("page", &page);
    ctx.insert("posts_per_page", &posts_per_page);
    ctx.insert("num_pages", &num_pages);

    if let Some(value) = get_flash_cookie::<FlashData>(&cookies) {
        ctx.insert("flash", &value);
    }

    let body = templates
        .render("index.html.tera", &ctx)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Template error"))?;

    Ok(Html(body))
}

pub async fn new_post(
    Extension(ref templates): Extension<Tera>,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    let ctx = tera::Context::new();
    let body = templates
        .render("new.html.tera", &ctx)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Template error"))?;

    Ok(Html(body))
}

// curl -X POST -H 'Content-Type: application/json' http://localhost:8000/api/ --data '{"title": "title11", "text":"text11","new_col":0}'
pub async fn api_create_post(
    Extension(ref conn): Extension<DatabaseConnection>,
    Json(input): Json<posts::Model>,
) -> impl IntoResponse {
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
pub async fn create_post(
    Extension(ref conn): Extension<DatabaseConnection>,
    form: Form<posts::Model>,
    mut cookies: Cookies,
) -> Result<PostResponse, (StatusCode, &'static str)> {
    let model = form.0;

    posts::ActiveModel {
        title: Set(model.title.to_owned()),
        text: Set(model.text.to_owned()),
        ..Default::default()
    }
    .save(conn)
    .await
    .expect("could not insert post");

    let data = FlashData {
        kind: "success".to_owned(),
        message: "Post succcessfully added".to_owned(),
    };

    Ok(post_response(&mut cookies, data))
}

pub async fn edit_post(
    Extension(ref templates): Extension<Tera>,
    Extension(ref conn): Extension<DatabaseConnection>,
    Path(id): Path<i32>,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    let post: posts::Model = Posts::find_by_id(id)
        .one(conn)
        .await
        .expect("could not find post")
        .unwrap();

    let mut ctx = tera::Context::new();
    ctx.insert("post", &post);

    let body = templates
        .render("edit.html.tera", &ctx)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Template error"))?;

    Ok(Html(body))
}

// curl -X PATCH -H 'Content-Type: application/json' http://localhost:8000/api/12 --data '{"title": "title11", "text":"text11","new_col":4}'
pub async fn api_update_post(
    Extension(ref conn): Extension<DatabaseConnection>,
    Path(id): Path<i32>,
    Json(input): Json<posts::Model>,
) -> impl IntoResponse {
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
pub async fn update_post(
    Extension(ref conn): Extension<DatabaseConnection>,
    Path(id): Path<i32>,
    form: Form<posts::Model>,
    mut cookies: Cookies,
) -> Result<PostResponse, (StatusCode, String)> {
    let model = form.0;

    posts::ActiveModel {
        id: Set(id),
        title: Set(model.title.to_owned()),
        text: Set(model.text.to_owned()),
        new_col: Set(model.new_col.to_owned()),
    }
    .save(conn)
    .await
    .expect("could not edit post");

    let data = FlashData {
        kind: "success".to_owned(),
        message: "Post succcessfully updated".to_owned(),
    };

    Ok(post_response(&mut cookies, data))
}

// curl -X DELETE  http://localhost:8000/api/12
pub async fn api_delete_post(
    Extension(ref conn): Extension<DatabaseConnection>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
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
pub async fn delete_post(
    Extension(ref conn): Extension<DatabaseConnection>,
    Path(id): Path<i32>,
    mut cookies: Cookies,
) -> Result<PostResponse, (StatusCode, &'static str)> {
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

    Ok(post_response(&mut cookies, data))
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
