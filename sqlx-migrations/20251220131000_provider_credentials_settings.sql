-- Provider credentials stored as provider-scoped settings.
-- Secrets must be provided via /run/secrets (never committed).
--
-- We store the Scaleway secret key as encrypted+base64 text in provider_settings.value_text:
--   encode(pgp_sym_encrypt(secret, passphrase), 'base64')
--
-- Decryption is done in-app using:
--   pgp_sym_decrypt(decode(value_text,'base64'), passphrase)

INSERT INTO public.settings_definitions (key, scope, value_type, description)
VALUES
  ('SCALEWAY_PROJECT_ID', 'provider', 'text', 'Scaleway project id used for Instance API calls.'),
  ('SCALEWAY_SECRET_KEY_ENC', 'provider', 'text', 'Base64(PGP_SYM_ENCRYPT(secret_key)). Seed from /run/secrets; do not edit manually.'),
  ('SCALEWAY_SECRET_KEY', 'provider', 'text', 'Plain Scaleway secret key (legacy / not recommended). Prefer SCALEWAY_SECRET_KEY_ENC.')
ON CONFLICT (key) DO UPDATE SET
  scope = EXCLUDED.scope,
  value_type = EXCLUDED.value_type,
  description = EXCLUDED.description;


