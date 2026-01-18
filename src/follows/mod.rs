use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod handler;

/// Database model for a follow relationship
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Follow {
    pub follower_id: Uuid,
    pub following_id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Response for a user in followers/following lists
#[derive(Debug, Serialize)]
pub struct FollowUserResponse {
    pub id: Uuid,
    pub username: String,
    pub bio: Option<String>,
    pub image: Option<String>,
    pub followed_at: chrono::DateTime<chrono::Utc>,
}

/// Query parameters for paginated follow lists
#[derive(Debug, Deserialize)]
pub struct FollowListFilter {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Response for paginated followers/following lists
#[derive(Debug, Serialize)]
pub struct FollowListResponse {
    pub users: Vec<FollowUserResponse>,
    pub total: i64,
    pub has_more: bool,
}

/// User profile with follow stats
#[derive(Debug, Serialize)]
pub struct UserProfileResponse {
    pub id: Uuid,
    pub username: String,
    pub bio: Option<String>,
    pub image: Option<String>,
    pub followers_count: i64,
    pub following_count: i64,
    pub is_following: bool, // Whether the current user follows this user
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Response for follow/unfollow actions
#[derive(Debug, Serialize)]
pub struct FollowActionResponse {
    pub following: bool,
    pub followers_count: i64,
}
