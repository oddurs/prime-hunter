-- User profiles with role-based access control.
-- Links Supabase auth.users to application roles (admin/operator).

CREATE TABLE user_profiles (
    id UUID PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'operator' CHECK (role IN ('admin', 'operator')),
    operator_id UUID REFERENCES operators(id) ON DELETE SET NULL,
    display_name TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_user_profiles_role ON user_profiles(role);
CREATE INDEX idx_user_profiles_operator_id ON user_profiles(operator_id);

ALTER TABLE user_profiles ENABLE ROW LEVEL SECURITY;

-- All authenticated users can read their own profile
CREATE POLICY "read_own_profile" ON user_profiles
    FOR SELECT USING (auth.uid() = id);

-- Admins can read all profiles
CREATE POLICY "admin_read_all_profiles" ON user_profiles
    FOR SELECT USING (
        EXISTS (SELECT 1 FROM user_profiles up WHERE up.id = auth.uid() AND up.role = 'admin')
    );

-- Users can update their own profile (display_name only, not role)
CREATE POLICY "update_own_profile" ON user_profiles
    FOR UPDATE USING (auth.uid() = id)
    WITH CHECK (auth.uid() = id);

-- Auto-create a profile when a new user signs up (via trigger)
CREATE OR REPLACE FUNCTION handle_new_user()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO user_profiles (id, role, display_name)
    VALUES (
        NEW.id,
        'operator',
        COALESCE(NEW.raw_user_meta_data->>'display_name', split_part(NEW.email, '@', 1))
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

CREATE TRIGGER on_auth_user_created
    AFTER INSERT ON auth.users
    FOR EACH ROW EXECUTE FUNCTION handle_new_user();
