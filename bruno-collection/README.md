# BlogVerse API - Bruno Collection

This is a Bruno collection for testing the BlogVerse API.

## Setup

1. Open Bruno
2. Click "Open Collection"
3. Navigate to this `bruno-collection` folder and open it
4. Select the "Local" environment from the environment dropdown

## Usage

### Authentication Flow

1. **Sign Up** - Create a new account (you'll need to verify email or manually set `email_verified = true` in the database)
2. **Sign In** - Login to get a JWT token (automatically saved to `{{token}}` variable)
3. Now all authenticated endpoints will work!

### Testing Comments

1. First, **Sign In** to get a token
2. **Create Story** to get a `storyId` (automatically saved)
3. **Create Comment** on the story (automatically saves `commentId`)
4. **Create Reply** to the comment
5. Test other comment operations (clap, update, delete)

## Endpoints

### Auth

- `POST /api/auth/sign-up` - Register a new user
- `POST /api/auth/sign-in` - Login and get JWT token
- `POST /api/auth/verify-email` - Verify email with token
- `POST /api/auth/resend-verification` - Resend verification email
- `POST /api/auth/forgot-password` - Request password reset
- `POST /api/auth/reset-password` - Reset password with token
- `GET /api/auth/me` - Get current user (requires auth)

### Users

- `GET /api/user/:id` - Get user by ID

### Stories

- `POST /api/stories` - Create a new story (requires auth)
- `GET /api/stories` - Get story feed (public)
- `GET /api/stories/s/:slug` - Get story by slug (public)
- `PUT /api/stories/:id` - Update story (requires auth, author only)
- `DELETE /api/stories/:id` - Delete story (requires auth, author only)
- `POST /api/stories/:id/clap` - Clap on story (requires auth)

### Comments

- `POST /api/stories/:id/comments` - Create comment on story (requires auth)
- `GET /api/stories/:id/comments` - Get story comments (public)
- `GET /api/comments/:id` - Get comment with replies (public)
- `GET /api/comments/:id/replies` - Get comment replies (public)
- `PUT /api/comments/:id` - Update comment (requires auth, author only)
- `DELETE /api/comments/:id` - Delete comment (requires auth, author only)
- `POST /api/comments/:id/clap` - Clap on comment (requires auth)

### Tags

- `GET /api/tags` - Get all tags (public)

## Variables

The collection uses these variables (stored in the Local environment):

| Variable    | Description        | Auto-set by                             |
| ----------- | ------------------ | --------------------------------------- |
| `baseUrl`   | API base URL       | Manual (default: http://localhost:8000) |
| `token`     | JWT token          | Sign In request                         |
| `storyId`   | Current story ID   | Create Story request                    |
| `commentId` | Current comment ID | Create Comment request                  |
