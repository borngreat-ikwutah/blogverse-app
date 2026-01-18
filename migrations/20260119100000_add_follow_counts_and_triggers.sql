-- Add denormalized follow counts to users table for better performance
ALTER TABLE users ADD COLUMN followers_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN following_count INTEGER NOT NULL DEFAULT 0;

-- Backfill existing counts
UPDATE users u SET followers_count = (
    SELECT COUNT(*) FROM follows f WHERE f.following_id = u.id
);

UPDATE users u SET following_count = (
    SELECT COUNT(*) FROM follows f WHERE f.follower_id = u.id
);

-- Create function to maintain follow counts automatically
CREATE OR REPLACE FUNCTION update_follow_counts()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        -- Increment follower count for the user being followed
        UPDATE users SET followers_count = followers_count + 1 WHERE id = NEW.following_id;
        -- Increment following count for the user who is following
        UPDATE users SET following_count = following_count + 1 WHERE id = NEW.follower_id;
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        -- Decrement follower count for the user being unfollowed
        UPDATE users SET followers_count = GREATEST(followers_count - 1, 0) WHERE id = OLD.following_id;
        -- Decrement following count for the user who unfollowed
        UPDATE users SET following_count = GREATEST(following_count - 1, 0) WHERE id = OLD.follower_id;
        RETURN OLD;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create trigger to automatically update counts on follow/unfollow
CREATE TRIGGER follow_count_trigger
AFTER INSERT OR DELETE ON follows
FOR EACH ROW EXECUTE FUNCTION update_follow_counts();

-- Add indexes for efficient sorting by popularity
CREATE INDEX idx_users_followers_count ON users(followers_count DESC);
CREATE INDEX idx_users_following_count ON users(following_count DESC);
