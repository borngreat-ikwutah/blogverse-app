use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use sqlx::{FromRow, PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    auth::jwt,
    error::AppError,
    follows::{
        FollowActionResponse, FollowListFilter, FollowListResponse, FollowUserResponse,
        UserProfileResponse,
    },
    response::ApiResponse,
    stories::{AuthorResponse, StoryResponse, StoryStatus},
};

/// Helper struct for fetching user with follow info
#[derive(FromRow)]
struct UserFollowRow {
    id: Uuid,
    username: String,
    bio: Option<String>,
    image: Option<String>,
    followed_at: chrono::DateTime<chrono::Utc>,
}

impl From<UserFollowRow> for FollowUserResponse {
    fn from(u: UserFollowRow) -> Self {
        FollowUserResponse {
            id: u.id,
            username: u.username,
            bio: u.bio,
            image: u.image,
            followed_at: u.followed_at,
        }
    }
}

/// Request payload for bulk follow status check
#[derive(Debug, Deserialize)]
pub struct BulkFollowCheckRequest {
    pub user_ids: Vec<Uuid>,
}

/// Follow a user
/// POST /api/users/:id/follow
pub async fn follow_user(
    State(pool): State<PgPool>,
    claims: jwt::Claims,
    Path(user_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // Can't follow yourself
    if claims.sub == user_id {
        return Err(AppError::UnprocessableEntity(
            "You cannot follow yourself".to_string(),
        ));
    }

    // Verify target user exists
    sqlx::query("SELECT id FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::NotFound("User not found".to_string()))?;

    // Insert follow (ignore if already following)
    // The trigger will automatically update the followers_count
    sqlx::query(
        r#"
        INSERT INTO follows (follower_id, following_id)
        VALUES ($1, $2)
        ON CONFLICT (follower_id, following_id) DO NOTHING
        "#,
    )
    .bind(claims.sub)
    .bind(user_id)
    .execute(&pool)
    .await
    .map_err(|_| AppError::InternalServerError)?;

    // Get the denormalized follower count directly from users table
    let count_row = sqlx::query("SELECT followers_count FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    let followers_count: i32 = count_row.get("followers_count");

    Ok(ApiResponse::success(FollowActionResponse {
        following: true,
        followers_count: followers_count as i64,
    }))
}

/// Unfollow a user
/// DELETE /api/users/:id/follow
pub async fn unfollow_user(
    State(pool): State<PgPool>,
    claims: jwt::Claims,
    Path(user_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // Verify target user exists
    sqlx::query("SELECT id FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::NotFound("User not found".to_string()))?;

    // Delete follow - the trigger will automatically update the followers_count
    sqlx::query("DELETE FROM follows WHERE follower_id = $1 AND following_id = $2")
        .bind(claims.sub)
        .bind(user_id)
        .execute(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    // Get the denormalized follower count directly from users table
    let count_row = sqlx::query("SELECT followers_count FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    let followers_count: i32 = count_row.get("followers_count");

    Ok(ApiResponse::success(FollowActionResponse {
        following: false,
        followers_count: followers_count as i64,
    }))
}

/// Get a user's followers
/// GET /api/users/:id/followers
pub async fn get_followers(
    State(pool): State<PgPool>,
    Path(user_id): Path<Uuid>,
    Query(filter): Query<FollowListFilter>,
) -> Result<impl IntoResponse, AppError> {
    // Get user and their denormalized follower count in one query
    let user_row = sqlx::query("SELECT id, followers_count FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::NotFound("User not found".to_string()))?;

    let total: i32 = user_row.get("followers_count");
    let total = total as i64;

    let limit = filter.limit.unwrap_or(20).min(100);
    let offset = filter.offset.unwrap_or(0);

    // Get followers with user info
    let followers = sqlx::query_as::<_, UserFollowRow>(
        r#"
        SELECT u.id, u.username, u.bio, u.image, f.created_at as followed_at
        FROM follows f
        JOIN users u ON f.follower_id = u.id
        WHERE f.following_id = $1
        ORDER BY f.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|_| AppError::InternalServerError)?;

    let users: Vec<FollowUserResponse> = followers
        .into_iter()
        .map(FollowUserResponse::from)
        .collect();
    let has_more = (offset + limit) < total;

    Ok(ApiResponse::success(FollowListResponse {
        users,
        total,
        has_more,
    }))
}

/// Get users that a user is following
/// GET /api/users/:id/following
pub async fn get_following(
    State(pool): State<PgPool>,
    Path(user_id): Path<Uuid>,
    Query(filter): Query<FollowListFilter>,
) -> Result<impl IntoResponse, AppError> {
    // Get user and their denormalized following count in one query
    let user_row = sqlx::query("SELECT id, following_count FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::NotFound("User not found".to_string()))?;

    let total: i32 = user_row.get("following_count");
    let total = total as i64;

    let limit = filter.limit.unwrap_or(20).min(100);
    let offset = filter.offset.unwrap_or(0);

    // Get following with user info
    let following = sqlx::query_as::<_, UserFollowRow>(
        r#"
        SELECT u.id, u.username, u.bio, u.image, f.created_at as followed_at
        FROM follows f
        JOIN users u ON f.following_id = u.id
        WHERE f.follower_id = $1
        ORDER BY f.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|_| AppError::InternalServerError)?;

    let users: Vec<FollowUserResponse> = following
        .into_iter()
        .map(FollowUserResponse::from)
        .collect();
    let has_more = (offset + limit) < total;

    Ok(ApiResponse::success(FollowListResponse {
        users,
        total,
        has_more,
    }))
}

/// Get user profile with follow stats
/// GET /api/users/:id/profile
pub async fn get_user_profile(
    State(pool): State<PgPool>,
    claims: Option<jwt::Claims>,
    Path(user_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // Get user with denormalized counts in a single query
    let user = sqlx::query(
        r#"
        SELECT id, username, bio, image, followers_count, following_count, created_at
        FROM users WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError)?
    .ok_or(AppError::NotFound("User not found".to_string()))?;

    let followers_count: i32 = user.get("followers_count");
    let following_count: i32 = user.get("following_count");

    // Check if current user follows this user
    let is_following = if let Some(claims) = claims {
        sqlx::query("SELECT 1 FROM follows WHERE follower_id = $1 AND following_id = $2")
            .bind(claims.sub)
            .bind(user_id)
            .fetch_optional(&pool)
            .await
            .map_err(|_| AppError::InternalServerError)?
            .is_some()
    } else {
        false
    };

    Ok(ApiResponse::success(UserProfileResponse {
        id: user.get("id"),
        username: user.get("username"),
        bio: user.get("bio"),
        image: user.get("image"),
        followers_count: followers_count as i64,
        following_count: following_count as i64,
        is_following,
        created_at: user.get("created_at"),
    }))
}

/// Helper struct for fetching stories with author info
#[derive(FromRow)]
struct StoryFromDb {
    id: Uuid,
    title: String,
    subtitle: Option<String>,
    content: serde_json::Value,
    slug: String,
    status: StoryStatus,
    clap_count: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
    author_id: Uuid,
    username: String,
    bio: Option<String>,
    image: Option<String>,
    tags: Vec<String>,
}

impl From<StoryFromDb> for StoryResponse {
    fn from(s: StoryFromDb) -> Self {
        StoryResponse {
            id: s.id,
            author: AuthorResponse {
                id: s.author_id,
                username: s.username,
                bio: s.bio,
                image: s.image,
            },
            title: s.title,
            subtitle: s.subtitle,
            content: s.content,
            slug: s.slug,
            status: s.status,
            clap_count: s.clap_count,
            tags: s.tags,
            created_at: s.created_at,
            published_at: s.published_at,
        }
    }
}

/// Get personalized feed (stories from followed users)
/// GET /api/feed/following
pub async fn get_following_feed(
    State(pool): State<PgPool>,
    claims: jwt::Claims,
    Query(filter): Query<crate::stories::StoryFilter>,
) -> Result<impl IntoResponse, AppError> {
    let limit = filter.limit.unwrap_or(20).min(100);
    let offset = filter.offset.unwrap_or(0);

    let order_clause = match filter.sort.as_deref() {
        Some("claps") => "s.clap_count DESC, s.created_at DESC",
        _ => "s.created_at DESC",
    };

    let query_str = format!(
        r#"
        SELECT
            s.id, s.title, s.subtitle, s.content, s.slug, s.status, s.clap_count,
            s.created_at, s.updated_at, s.published_at, s.author_id,
            u.username, u.bio, u.image,
            COALESCE(ARRAY_AGG(t.name) FILTER (WHERE t.name IS NOT NULL), '{{}}') as tags
        FROM stories s
        JOIN users u ON s.author_id = u.id
        JOIN follows f ON s.author_id = f.following_id AND f.follower_id = $1
        LEFT JOIN story_tags st ON s.id = st.story_id
        LEFT JOIN tags t ON st.tag_id = t.id
        WHERE s.status = 'published'
        GROUP BY s.id, u.id
        ORDER BY {}
        LIMIT $2 OFFSET $3
        "#,
        order_clause
    );

    let stories = sqlx::query_as::<_, StoryFromDb>(&query_str)
        .bind(claims.sub)
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Following feed error: {:?}", e);
            AppError::InternalServerError
        })?;

    let response: Vec<StoryResponse> = stories.into_iter().map(StoryResponse::from).collect();

    Ok(ApiResponse::success(response))
}

/// Check if current user follows a target user
/// GET /api/users/:id/is-following
pub async fn check_following(
    State(pool): State<PgPool>,
    claims: jwt::Claims,
    Path(user_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let is_following =
        sqlx::query("SELECT 1 FROM follows WHERE follower_id = $1 AND following_id = $2")
            .bind(claims.sub)
            .bind(user_id)
            .fetch_optional(&pool)
            .await
            .map_err(|_| AppError::InternalServerError)?
            .is_some();

    Ok(ApiResponse::success(
        serde_json::json!({ "following": is_following }),
    ))
}

/// Check follow status for multiple users in a single request
/// POST /api/user/following-status
///
/// This is much more efficient than making individual is-following requests
/// when rendering a list of users (e.g., search results, suggested users)
pub async fn check_following_bulk(
    State(pool): State<PgPool>,
    claims: jwt::Claims,
    Json(payload): Json<BulkFollowCheckRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Limit the number of user IDs to prevent abuse
    if payload.user_ids.len() > 100 {
        return Err(AppError::UnprocessableEntity(
            "Maximum 100 user IDs allowed per request".to_string(),
        ));
    }

    if payload.user_ids.is_empty() {
        return Ok(ApiResponse::success(HashMap::<Uuid, bool>::new()));
    }

    // Get all users that the current user follows from the provided list
    let following_ids = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT following_id FROM follows
        WHERE follower_id = $1 AND following_id = ANY($2)
        "#,
    )
    .bind(claims.sub)
    .bind(&payload.user_ids)
    .fetch_all(&pool)
    .await
    .map_err(|_| AppError::InternalServerError)?;

    // Build a HashMap with follow status for each requested user
    let result: HashMap<Uuid, bool> = payload
        .user_ids
        .into_iter()
        .map(|id| (id, following_ids.contains(&id)))
        .collect();

    Ok(ApiResponse::success(result))
}

/// Get suggested users to follow based on mutual connections
/// GET /api/user/suggestions
///
/// Returns users who are followed by people you follow, but you don't follow yet
pub async fn get_follow_suggestions(
    State(pool): State<PgPool>,
    claims: jwt::Claims,
    Query(filter): Query<FollowListFilter>,
) -> Result<impl IntoResponse, AppError> {
    let limit = filter.limit.unwrap_or(10).min(50);

    // Find users followed by people you follow, that you don't already follow
    // Ordered by how many mutual connections they have
    let suggestions = sqlx::query_as::<_, SuggestionRow>(
        r#"
        SELECT
            u.id, u.username, u.bio, u.image, u.followers_count,
            COUNT(DISTINCT f2.follower_id) as mutual_count
        FROM follows f1
        JOIN follows f2 ON f1.following_id = f2.follower_id
        JOIN users u ON f2.following_id = u.id
        WHERE f1.follower_id = $1
          AND f2.following_id != $1
          AND NOT EXISTS (
              SELECT 1 FROM follows
              WHERE follower_id = $1 AND following_id = f2.following_id
          )
        GROUP BY u.id
        ORDER BY mutual_count DESC, u.followers_count DESC
        LIMIT $2
        "#,
    )
    .bind(claims.sub)
    .bind(limit)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        tracing::error!("Follow suggestions error: {:?}", e);
        AppError::InternalServerError
    })?;

    let response: Vec<FollowSuggestionResponse> = suggestions
        .into_iter()
        .map(FollowSuggestionResponse::from)
        .collect();

    Ok(ApiResponse::success(response))
}

/// Helper struct for follow suggestions query
#[derive(FromRow)]
struct SuggestionRow {
    id: Uuid,
    username: String,
    bio: Option<String>,
    image: Option<String>,
    followers_count: i32,
    mutual_count: i64,
}

/// Response for follow suggestions
#[derive(Debug, serde::Serialize)]
pub struct FollowSuggestionResponse {
    pub id: Uuid,
    pub username: String,
    pub bio: Option<String>,
    pub image: Option<String>,
    pub followers_count: i64,
    pub mutual_followers_count: i64,
}

impl From<SuggestionRow> for FollowSuggestionResponse {
    fn from(s: SuggestionRow) -> Self {
        FollowSuggestionResponse {
            id: s.id,
            username: s.username,
            bio: s.bio,
            image: s.image,
            followers_count: s.followers_count as i64,
            mutual_followers_count: s.mutual_count,
        }
    }
}
