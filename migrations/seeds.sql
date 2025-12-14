-- NOTE: catalog seeding is centralized in `seeds/catalog_seeds.sql`.
-- This file only keeps non-catalog dev data.

-- Initial Admin User (Default pwd: password)
-- Note: In real app, use ARGON2 hash. Here is a placeholder hash.
INSERT INTO users (id, email, password_hash, role)
VALUES ('00000000-0000-0000-0000-000000000099', 'hammed.ramdani@inventiv-it.fr', '$argon2id$v=19$m=4096,t=3,p=1$placeholder$placeholder', 'admin')
ON CONFLICT (email) DO NOTHING;
