CREATE TYPE story_status AS ENUM ('draft', 'published');

CREATE TABLE stories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    author_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title VARCHAR(255) NOT NULL,
    subtitle VARCHAR(255),
    content JSONB NOT NULL,
    slug VARCHAR(255) NOT NULL UNIQUE,
    status story_status NOT NULL DEFAULT 'draft',
    clap_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    published_at TIMESTAMPTZ
);

CREATE INDEX idx_stories_slug ON stories(slug);
CREATE INDEX idx_stories_author ON stories(author_id);
CREATE INDEX idx_stories_created_at ON stories(created_at);
CREATE INDEX idx_stories_clap_count ON stories(clap_count);

CREATE TABLE tags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(50) NOT NULL UNIQUE
);

CREATE TABLE story_tags (
    story_id UUID NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (story_id, tag_id)
);

CREATE TABLE story_claps (
    story_id UUID NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    claps_count INTEGER NOT NULL DEFAULT 1 CHECK (claps_count <= 50),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (story_id, user_id)
);
