use serde::{Deserialize, Serialize};
use sqlx::prelude::Type;
use uuid::Uuid;
use validator::Validate;

pub mod handler;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Story {
    pub id: Uuid,
    pub author_id: Uuid,
    pub title: String,
    pub subtitle: Option<String>,
    pub content: serde_json::Value,
    pub slug: String,
    pub status: StoryStatus,
    pub clap_count: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[sqlx(type_name = "story_status", rename_all = "lowercase")]
pub enum StoryStatus {
    Draft,
    Published,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Tag {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateStory {
    #[validate(length(min = 1, message = "Title cannot be empty"))]
    pub title: String,
    pub subtitle: Option<String>,
    pub content: serde_json::Value,
    pub tags: Vec<String>,
    pub publish: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateStory {
    #[validate(length(min = 1, message = "Title cannot be empty"))]
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub content: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
    pub publish: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct StoryResponse {
    pub id: Uuid,
    pub author: AuthorResponse,
    pub title: String,
    pub subtitle: Option<String>,
    pub content: serde_json::Value,
    pub slug: String,
    pub status: StoryStatus,
    pub clap_count: i32,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct AuthorResponse {
    pub id: Uuid,
    pub username: String,
    pub bio: Option<String>,
    pub image: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StoryFilter {
    pub tag: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub sort: Option<String>, // "latest" or "claps"
}
