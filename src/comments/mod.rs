use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

pub mod handler;

/// Database model for a comment
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Comment {
    pub id: Uuid,
    pub story_id: Uuid,
    pub author_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub content: String,
    pub clap_count: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Request payload for creating a comment
#[derive(Debug, Deserialize, Validate)]
pub struct CreateComment {
    #[validate(length(
        min = 1,
        max = 10000,
        message = "Comment must be between 1 and 10000 characters"
    ))]
    pub content: String,
    pub parent_id: Option<Uuid>, // Optional: for nested replies
}

/// Request payload for updating a comment
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateComment {
    #[validate(length(
        min = 1,
        max = 10000,
        message = "Comment must be between 1 and 10000 characters"
    ))]
    pub content: String,
}

/// Response structure for a comment with author info
#[derive(Debug, Serialize)]
pub struct CommentResponse {
    pub id: Uuid,
    pub story_id: Uuid,
    pub author: CommentAuthor,
    pub parent_id: Option<Uuid>,
    pub content: String,
    pub clap_count: i32,
    pub replies_count: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Author info embedded in comment response
#[derive(Debug, Serialize)]
pub struct CommentAuthor {
    pub id: Uuid,
    pub username: String,
    pub image: Option<String>,
}

/// Query parameters for fetching comments
#[derive(Debug, Deserialize)]
pub struct CommentFilter {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub sort: Option<String>, // "latest", "oldest", or "claps"
}

/// Response for paginated comments list
#[derive(Debug, Serialize)]
pub struct CommentsListResponse {
    pub comments: Vec<CommentResponse>,
    pub total: i64,
    pub has_more: bool,
}

/// Nested comment structure with replies
#[derive(Debug, Serialize)]
pub struct CommentWithReplies {
    #[serde(flatten)]
    pub comment: CommentResponse,
    pub replies: Vec<CommentResponse>,
}
