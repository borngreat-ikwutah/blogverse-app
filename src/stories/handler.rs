use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use slug::slugify;
use sqlx::{FromRow, PgPool, Row};
use uuid::Uuid;
use validator::Validate;

use crate::{
    auth::jwt,
    error::AppError,
    response::ApiResponse,
    stories::{
        AuthorResponse, CreateStory, Story, StoryFilter, StoryResponse, StoryStatus, UpdateStory,
    },
};

pub async fn create_story(
    State(pool): State<PgPool>,
    claims: jwt::Claims,
    Json(payload): Json<CreateStory>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    let slug_base = slugify(&payload.title);
    let mut slug = slug_base.clone();
    let mut suffix = 1;

    // Simple slug uniqueness check
    while sqlx::query("SELECT 1 FROM stories WHERE slug = $1")
        .bind(&slug)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .is_some()
    {
        slug = format!("{}-{}", slug_base, suffix);
        suffix += 1;
    }

    let status = if payload.publish {
        StoryStatus::Published
    } else {
        StoryStatus::Draft
    };

    let now = chrono::Utc::now();
    let published_at = if payload.publish { Some(now) } else { None };

    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError)?;

    let story = sqlx::query_as::<_, Story>(
        r#"
        INSERT INTO stories (author_id, title, subtitle, content, slug, status, published_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING *
        "#,
    )
    .bind(claims.sub)
    .bind(&payload.title)
    .bind(&payload.subtitle)
    .bind(&payload.content)
    .bind(&slug)
    .bind(&status)
    .bind(published_at)
    .bind(now)
    .bind(now)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create story: {:?}", e);
        AppError::InternalServerError
    })?;

    // Handle tags
    for tag_name in payload.tags {
        let tag_clean = tag_name.trim().to_lowercase();

        // Ensure tag exists and get its id
        let tag_row = sqlx::query(
            "INSERT INTO tags (name) VALUES ($1) ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name RETURNING id",
        )
        .bind(&tag_clean)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError)?;

        let tag_id: Uuid = tag_row.get("id");

        // Link tag
        sqlx::query(
            "INSERT INTO story_tags (story_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(story.id)
        .bind(tag_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError)?;
    }

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError)?;

    // Fetch complete story with tags and author
    get_story_response(&pool, story.id).await
}

pub async fn get_story(
    State(pool): State<PgPool>,
    Path(slug): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let row = sqlx::query("SELECT id FROM stories WHERE slug = $1")
        .bind(&slug)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::NotFound("Story not found".to_string()))?;

    let story_id: Uuid = row.get("id");

    get_story_response(&pool, story_id).await
}

pub async fn update_story(
    State(pool): State<PgPool>,
    claims: jwt::Claims,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateStory>,
) -> Result<impl IntoResponse, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError)?;

    // Verify ownership
    let row = sqlx::query("SELECT author_id FROM stories WHERE id = $1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::NotFound("Story not found".to_string()))?;

    let story_author_id: Uuid = row.get("author_id");

    if story_author_id != claims.sub {
        return Err(AppError::Unauthorized);
    }

    if let Some(title) = &payload.title {
        let _ = sqlx::query(
            "UPDATE stories SET title = $1, slug = $2, updated_at = NOW() WHERE id = $3",
        )
        .bind(title)
        .bind(slugify(title))
        .bind(id)
        .execute(&mut *tx)
        .await;
    }

    if let Some(subtitle) = &payload.subtitle {
        let _ = sqlx::query("UPDATE stories SET subtitle = $1, updated_at = NOW() WHERE id = $2")
            .bind(subtitle)
            .bind(id)
            .execute(&mut *tx)
            .await;
    }

    if let Some(content) = &payload.content {
        let _ = sqlx::query("UPDATE stories SET content = $1, updated_at = NOW() WHERE id = $2")
            .bind(content)
            .bind(id)
            .execute(&mut *tx)
            .await;
    }

    if let Some(publish) = payload.publish {
        let status = if publish {
            StoryStatus::Published
        } else {
            StoryStatus::Draft
        };
        let published_at = if publish {
            Some(chrono::Utc::now())
        } else {
            None
        };
        let _ = sqlx::query(
            "UPDATE stories SET status = $1, published_at = COALESCE(published_at, $2), updated_at = NOW() WHERE id = $3",
        )
        .bind(&status)
        .bind(published_at)
        .bind(id)
        .execute(&mut *tx)
        .await;
    }

    if let Some(tags) = payload.tags {
        // Clear existing tags
        sqlx::query("DELETE FROM story_tags WHERE story_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|_| AppError::InternalServerError)?;

        // Re-add tags
        for tag_name in tags {
            let tag_clean = tag_name.trim().to_lowercase();
            let tag_row = sqlx::query(
                "INSERT INTO tags (name) VALUES ($1) ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name RETURNING id",
            )
            .bind(&tag_clean)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| AppError::InternalServerError)?;

            let tag_id: Uuid = tag_row.get("id");

            sqlx::query(
                "INSERT INTO story_tags (story_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(id)
            .bind(tag_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| AppError::InternalServerError)?;
        }
    }

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError)?;

    get_story_response(&pool, id).await
}

pub async fn delete_story(
    State(pool): State<PgPool>,
    claims: jwt::Claims,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let row = sqlx::query("SELECT author_id FROM stories WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::NotFound("Story not found".to_string()))?;

    let story_author_id: Uuid = row.get("author_id");

    if story_author_id != claims.sub {
        return Err(AppError::Unauthorized);
    }

    sqlx::query("DELETE FROM stories WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    Ok(ApiResponse::ok("Story deleted".to_string()))
}

pub async fn get_feed(
    State(pool): State<PgPool>,
    Query(filter): Query<StoryFilter>,
) -> Result<impl IntoResponse, AppError> {
    let limit = filter.limit.unwrap_or(20).min(100);
    let offset = filter.offset.unwrap_or(0);

    // Sort
    let order_clause = match filter.sort.as_deref() {
        Some("claps") => "s.clap_count DESC, s.created_at DESC",
        _ => "s.created_at DESC", // Default latest
    };

    // Filter by tag
    let where_clause = if let Some(tag) = &filter.tag {
        format!("WHERE t.name = '{}' AND s.status = 'published'", tag)
    } else {
        "WHERE s.status = 'published'".to_string()
    };

    let query_str = format!(
        r#"
        SELECT 
            s.id, s.title, s.subtitle, s.content, s.slug, s.status, s.clap_count, s.created_at as created_at, s.updated_at, s.published_at, s.author_id,
            u.username, u.bio, u.image,
            COALESCE(ARRAY_AGG(t.name) FILTER (WHERE t.name IS NOT NULL), '{{}}') as tags
        FROM stories s
        JOIN users u ON s.author_id = u.id
        LEFT JOIN story_tags st ON s.id = st.story_id
        LEFT JOIN tags t ON st.tag_id = t.id
        {}
        GROUP BY s.id, u.id
        ORDER BY {}
        LIMIT $1 OFFSET $2
        "#,
        where_clause, order_clause
    );

    let rows = sqlx::query_as::<_, StoryFromDb>(&query_str)
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Feed error: {:?}", e);
            AppError::InternalServerError
        })?;

    let response: Vec<StoryResponse> = rows.into_iter().map(StoryResponse::from).collect();

    Ok(ApiResponse::success(response))
}

pub async fn clap_story(
    State(pool): State<PgPool>,
    claims: jwt::Claims,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError)?;

    // Check if story exists and is published
    sqlx::query("SELECT id FROM stories WHERE id = $1 AND status = 'published'")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError)?
        .ok_or(AppError::NotFound("Story not found".to_string()))?;

    // Check existing claps
    let current_claps_row =
        sqlx::query("SELECT claps_count FROM story_claps WHERE story_id = $1 AND user_id = $2")
            .bind(id)
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
        INSERT INTO story_claps (story_id, user_id, claps_count)
        VALUES ($1, $2, 1)
        ON CONFLICT (story_id, user_id)
        DO UPDATE SET claps_count = story_claps.claps_count + 1, updated_at = NOW()
        "#,
    )
    .bind(id)
    .bind(claims.sub)
    .execute(&mut *tx)
    .await
    .map_err(|_| AppError::InternalServerError)?;

    // Update total claps on story
    sqlx::query("UPDATE stories SET clap_count = clap_count + 1 WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError)?;

    Ok(ApiResponse::ok("Clap recorded".to_string()))
}

pub async fn get_tags(State(pool): State<PgPool>) -> Result<impl IntoResponse, AppError> {
    let tags = sqlx::query_as::<_, crate::stories::Tag>("SELECT * FROM tags ORDER BY name ASC")
        .fetch_all(&pool)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    Ok(ApiResponse::success(tags))
}

// Helper struct and function
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
    // author fields
    author_id: Uuid,
    username: String,
    bio: Option<String>,
    image: Option<String>,
    // tags
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

async fn get_story_response(
    pool: &PgPool,
    story_id: Uuid,
) -> Result<ApiResponse<StoryResponse>, AppError> {
    let row = sqlx::query_as::<_, StoryFromDb>(
        r#"
        SELECT 
            s.id, s.title, s.subtitle, s.content, s.slug, s.status, s.clap_count, s.created_at, s.published_at,
            u.id as author_id, u.username, u.bio, u.image,
            COALESCE(ARRAY_AGG(t.name) FILTER (WHERE t.name IS NOT NULL), '{}') as tags
        FROM stories s
        JOIN users u ON s.author_id = u.id
        LEFT JOIN story_tags st ON s.id = st.story_id
        LEFT JOIN tags t ON st.tag_id = t.id
        WHERE s.id = $1
        GROUP BY s.id, u.id
        "#,
    )
    .bind(story_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Fetch story error: {:?}", e);
        AppError::InternalServerError
    })?
    .ok_or(AppError::NotFound("Story not found".to_string()))?;

    Ok(ApiResponse::success(StoryResponse::from(row)))
}
