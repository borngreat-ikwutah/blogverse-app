use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use sqlx::{FromRow, PgPool, Row};
use uuid::Uuid;
use validator::Validate;

use crate::{
    auth::jwt,
    comments::{
        CommentAuthor, CommentFilter, CommentResponse, CommentWithReplies, CommentsListResponse,
        CreateComment, UpdateComment,
    },
    error::AppError,
    response::ApiResponse,
};

/// Helper struct for fetching comments with author info from database
#[derive(FromRow)]
struct CommentFromDb {
    id: Uuid,
    story_id: Uuid,
    author_id: Uuid,
    parent_id: Option<Uuid>,
    content: String,
    clap_count: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    // Author fields
    username: String,
    image: Option<String>,
    // Replies count
    replies_count: i64,
}

impl From<CommentFromDb> for CommentResponse {
    fn from(c: CommentFromDb) -> Self {
        CommentResponse {
            id: c.id,
            story_id: c.story_id,
            author: CommentAuthor {
                id: c.author_id,
                username: c.username,
                image: c.image,
            },
            parent_id: c.parent_id,
            content: c.content,
            clap_count: c.clap_count,
            replies_count: c.replies_count,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

/// Create a new comment on a story
/// POST /api/stories/:id/comments
pub async fn create_comment(
    State(pool): State<PgPool>,
    claims: jwt::Claims,
    Path(story_id): Path<Uuid>,
    Json(payload): Json<CreateComment>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    // Verify story exists and is published
    sqlx::query("SELECT id FROM stories WHERE id = $1 AND status = 'published'")
        .bind(story_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::NotFound("Story not found".to_string()))?;

    // If replying to a comment, verify parent exists and belongs to same story
    if let Some(parent_id) = payload.parent_id {
        let parent = sqlx::query("SELECT story_id FROM comments WHERE id = $1")
            .bind(parent_id)
            .fetch_optional(&pool)
            .await
            .map_err(|_| AppError::InternalServerError)?
            .ok_or(AppError::NotFound("Parent comment not found".to_string()))?;

        let parent_story_id: Uuid = parent.get("story_id");
        if parent_story_id != story_id {
            return Err(AppError::UnprocessableEntity(
                "Parent comment does not belong to this story".to_string(),
            ));
        }
    }

    let now = chrono::Utc::now();

    let comment = sqlx::query_as::<_, crate::comments::Comment>(
        r#"
        INSERT INTO comments (story_id, author_id, parent_id, content, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#,
    )
    .bind(story_id)
    .bind(claims.sub)
    .bind(payload.parent_id)
    .bind(&payload.content)
    .bind(now)
    .bind(now)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create comment: {:?}", e);
        AppError::InternalServerError
    })?;

    // Fetch the complete comment with author info
    get_comment_response(&pool, comment.id).await
}

/// Get all top-level comments for a story (with replies count)
/// GET /api/stories/:id/comments
pub async fn get_story_comments(
    State(pool): State<PgPool>,
    Path(story_id): Path<Uuid>,
    Query(filter): Query<CommentFilter>,
) -> Result<impl IntoResponse, AppError> {
    // Verify story exists
    sqlx::query("SELECT id FROM stories WHERE id = $1")
        .bind(story_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::NotFound("Story not found".to_string()))?;

    let limit = filter.limit.unwrap_or(20).min(100);
    let offset = filter.offset.unwrap_or(0);

    let order_clause = match filter.sort.as_deref() {
        Some("oldest") => "c.created_at ASC",
        Some("claps") => "c.clap_count DESC, c.created_at DESC",
        _ => "c.created_at DESC", // Default: latest
    };

    // Get total count of top-level comments
    let total_row = sqlx::query(
        "SELECT COUNT(*) as count FROM comments WHERE story_id = $1 AND parent_id IS NULL",
    )
    .bind(story_id)
    .fetch_one(&pool)
    .await
    .map_err(|_| AppError::InternalServerError)?;

    let total: i64 = total_row.get("count");

    // Fetch top-level comments with author info and replies count
    let query_str = format!(
        r#"
        SELECT 
            c.id, c.story_id, c.author_id, c.parent_id, c.content, c.clap_count, 
            c.created_at, c.updated_at,
            u.username, u.image,
            (SELECT COUNT(*) FROM comments WHERE parent_id = c.id) as replies_count
        FROM comments c
        JOIN users u ON c.author_id = u.id
        WHERE c.story_id = $1 AND c.parent_id IS NULL
        ORDER BY {}
        LIMIT $2 OFFSET $3
        "#,
        order_clause
    );

    let comments = sqlx::query_as::<_, CommentFromDb>(&query_str)
        .bind(story_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch comments: {:?}", e);
            AppError::InternalServerError
        })?;

    let comments_response: Vec<CommentResponse> =
        comments.into_iter().map(CommentResponse::from).collect();

    let has_more = (offset + limit) < total;

    Ok(ApiResponse::success(CommentsListResponse {
        comments: comments_response,
        total,
        has_more,
    }))
}

/// Get replies to a specific comment
/// GET /api/comments/:id/replies
pub async fn get_comment_replies(
    State(pool): State<PgPool>,
    Path(comment_id): Path<Uuid>,
    Query(filter): Query<CommentFilter>,
) -> Result<impl IntoResponse, AppError> {
    // Verify parent comment exists
    sqlx::query("SELECT id FROM comments WHERE id = $1")
        .bind(comment_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::NotFound("Comment not found".to_string()))?;

    let limit = filter.limit.unwrap_or(20).min(100);
    let offset = filter.offset.unwrap_or(0);

    let order_clause = match filter.sort.as_deref() {
        Some("oldest") => "c.created_at ASC",
        Some("claps") => "c.clap_count DESC, c.created_at DESC",
        _ => "c.created_at DESC",
    };

    // Get total count of replies
    let total_row = sqlx::query("SELECT COUNT(*) as count FROM comments WHERE parent_id = $1")
        .bind(comment_id)
        .fetch_one(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    let total: i64 = total_row.get("count");

    let query_str = format!(
        r#"
        SELECT 
            c.id, c.story_id, c.author_id, c.parent_id, c.content, c.clap_count, 
            c.created_at, c.updated_at,
            u.username, u.image,
            (SELECT COUNT(*) FROM comments WHERE parent_id = c.id) as replies_count
        FROM comments c
        JOIN users u ON c.author_id = u.id
        WHERE c.parent_id = $1
        ORDER BY {}
        LIMIT $2 OFFSET $3
        "#,
        order_clause
    );

    let replies = sqlx::query_as::<_, CommentFromDb>(&query_str)
        .bind(comment_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch replies: {:?}", e);
            AppError::InternalServerError
        })?;

    let replies_response: Vec<CommentResponse> =
        replies.into_iter().map(CommentResponse::from).collect();

    let has_more = (offset + limit) < total;

    Ok(ApiResponse::success(CommentsListResponse {
        comments: replies_response,
        total,
        has_more,
    }))
}

/// Get a single comment with its replies (threaded view)
/// GET /api/comments/:id
pub async fn get_comment(
    State(pool): State<PgPool>,
    Path(comment_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // Fetch the comment
    let comment = sqlx::query_as::<_, CommentFromDb>(
        r#"
        SELECT 
            c.id, c.story_id, c.author_id, c.parent_id, c.content, c.clap_count, 
            c.created_at, c.updated_at,
            u.username, u.image,
            (SELECT COUNT(*) FROM comments WHERE parent_id = c.id) as replies_count
        FROM comments c
        JOIN users u ON c.author_id = u.id
        WHERE c.id = $1
        "#,
    )
    .bind(comment_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError)?
    .ok_or(AppError::NotFound("Comment not found".to_string()))?;

    // Fetch first few replies
    let replies = sqlx::query_as::<_, CommentFromDb>(
        r#"
        SELECT 
            c.id, c.story_id, c.author_id, c.parent_id, c.content, c.clap_count, 
            c.created_at, c.updated_at,
            u.username, u.image,
            (SELECT COUNT(*) FROM comments WHERE parent_id = c.id) as replies_count
        FROM comments c
        JOIN users u ON c.author_id = u.id
        WHERE c.parent_id = $1
        ORDER BY c.created_at ASC
        LIMIT 5
        "#,
    )
    .bind(comment_id)
    .fetch_all(&pool)
    .await
    .map_err(|_| AppError::InternalServerError)?;

    let comment_response = CommentResponse::from(comment);
    let replies_response: Vec<CommentResponse> =
        replies.into_iter().map(CommentResponse::from).collect();

    Ok(ApiResponse::success(CommentWithReplies {
        comment: comment_response,
        replies: replies_response,
    }))
}

/// Update a comment (author only)
/// PUT /api/comments/:id
pub async fn update_comment(
    State(pool): State<PgPool>,
    claims: jwt::Claims,
    Path(comment_id): Path<Uuid>,
    Json(payload): Json<UpdateComment>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    // Verify ownership
    let row = sqlx::query("SELECT author_id FROM comments WHERE id = $1")
        .bind(comment_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::NotFound("Comment not found".to_string()))?;

    let author_id: Uuid = row.get("author_id");
    if author_id != claims.sub {
        return Err(AppError::Unauthorized);
    }

    sqlx::query("UPDATE comments SET content = $1, updated_at = NOW() WHERE id = $2")
        .bind(&payload.content)
        .bind(comment_id)
        .execute(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    get_comment_response(&pool, comment_id).await
}

/// Delete a comment (author only)
/// DELETE /api/comments/:id
pub async fn delete_comment(
    State(pool): State<PgPool>,
    claims: jwt::Claims,
    Path(comment_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // Verify ownership
    let row = sqlx::query("SELECT author_id FROM comments WHERE id = $1")
        .bind(comment_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::NotFound("Comment not found".to_string()))?;

    let author_id: Uuid = row.get("author_id");
    if author_id != claims.sub {
        return Err(AppError::Unauthorized);
    }

    // Delete comment (cascades to replies due to FK constraint)
    sqlx::query("DELETE FROM comments WHERE id = $1")
        .bind(comment_id)
        .execute(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    Ok(ApiResponse::ok("Comment deleted".to_string()))
}

/// Clap on a comment
/// POST /api/comments/:id/clap
pub async fn clap_comment(
    State(pool): State<PgPool>,
    claims: jwt::Claims,
    Path(comment_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError)?;

    // Check if comment exists
    sqlx::query("SELECT id FROM comments WHERE id = $1")
        .bind(comment_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::NotFound("Comment not found".to_string()))?;

    // Check existing claps
    let current_claps_row =
        sqlx::query("SELECT claps_count FROM comment_claps WHERE comment_id = $1 AND user_id = $2")
            .bind(comment_id)
            .bind(claims.sub)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| AppError::InternalServerError)?;

    let current_count: i32 = current_claps_row.map(|r| r.get("claps_count")).unwrap_or(0);

    if current_count >= 50 {
        return Err(AppError::UnprocessableEntity(
            "Max 50 claps allowed".to_string(),
        ));
    }

    // Upsert clap count
    sqlx::query(
        r#"
        INSERT INTO comment_claps (comment_id, user_id, claps_count)
        VALUES ($1, $2, 1)
        ON CONFLICT (comment_id, user_id)
        DO UPDATE SET claps_count = comment_claps.claps_count + 1, updated_at = NOW()
        "#,
    )
    .bind(comment_id)
    .bind(claims.sub)
    .execute(&mut *tx)
    .await
    .map_err(|_| AppError::InternalServerError)?;

    // Update total claps on comment
    sqlx::query("UPDATE comments SET clap_count = clap_count + 1 WHERE id = $1")
        .bind(comment_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError)?;

    // Return updated comment
    get_comment_response(&pool, comment_id).await
}

/// Helper function to fetch a single comment with full details
async fn get_comment_response(
    pool: &PgPool,
    comment_id: Uuid,
) -> Result<ApiResponse<CommentResponse>, AppError> {
    let comment = sqlx::query_as::<_, CommentFromDb>(
        r#"
        SELECT 
            c.id, c.story_id, c.author_id, c.parent_id, c.content, c.clap_count, 
            c.created_at, c.updated_at,
            u.username, u.image,
            (SELECT COUNT(*) FROM comments WHERE parent_id = c.id) as replies_count
        FROM comments c
        JOIN users u ON c.author_id = u.id
        WHERE c.id = $1
        "#,
    )
    .bind(comment_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch comment: {:?}", e);
        AppError::InternalServerError
    })?
    .ok_or(AppError::NotFound("Comment not found".to_string()))?;

    Ok(ApiResponse::success(CommentResponse::from(comment)))
}
