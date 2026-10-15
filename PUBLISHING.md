# Publishing API

Run `cargo migrate` before starting the upgraded API. The migration adds document
columns with defaults, comments, identities, and sessions without dropping existing
tables. `cargo migrate -- --seed` also inserts seed drafts when empty. Migration
can be rerun safely. Back up the database before deploying schema changes.

Admin sign-in uses GitHub or Google OAuth. Configure the provider client ID and
secret, then set `ADMIN_GITHUB_IDS` to comma-separated numeric GitHub user IDs
and/or `ADMIN_GOOGLE_SUBJECTS` to Google's immutable `sub` values. Empty lists
deny all accounts. Usernames and email addresses never grant admin access.
Google profiles must have a verified email before their subject is considered.
Use your approved Google account on its first sign-in; the rejection page shows
its account ID for explicit owner approval. Add that subject and sign in again.

Register callbacks at `https://www.voidreg.xyz/api/v1/auth/github/callback` and
`https://www.voidreg.xyz/api/v1/auth/google/callback` for production. Set
`PUBLIC_BASE_URL=https://www.voidreg.xyz`. The admin button starts OAuth with
`purpose=admin`; the purpose and provider are bound into the checked state.
Commenter sign-in never promotes a session, even for an allowlisted owner.
Every admin request rechecks the current account allowlist. Removing an ID and
restarting the service immediately denies its existing sessions.

`GET /api/v1/admin/login-options` exposes configured providers and whether local
password login is enabled, without exposing credentials or allowlists. Admin
OAuth creates a separate HttpOnly session cookie and a session-bound CSRF token.
Unsafe admin requests send `X-CSRF-Token` from the `voidreg_admin_csrf` cookie.
`GET /api/v1/admin/session` restores a session; `DELETE` revokes it. Admin
responses carry `Cache-Control: no-store`. HTTPS origins enable Secure cookies.

Password login is disabled unless `ADMIN_PASSWORD_ENABLED=true`. It remains
available for explicit local development with `ADMIN_PASSWORD` or an Argon2
`ADMIN_PASSWORD_HASH`. Single-quote PHC hashes in Compose env files to preserve
dollar signs. Disabling password login also rejects existing password sessions.
Do not remove the working production login until OAuth credentials, allowlists,
and an owner sign-in have been verified. Then disable the flag and remove the
password/hash from production configuration.

Set a private `COMMENT_IP_SALT`. Use explicit `CORS_ORIGINS` including your
primary site origin for credentialed requests.

Article CRUD uses `/articles` and `/articles/{slug}`. Slugs are immutable.
`POST /articles/{slug}/publish` and `/unpublish` change visibility. Public queries
never return drafts. Existing plain-text sections become canonical Publisle
blocks on their first editor save. To publish, provide a schema-v1 document and a
format-v1 `publisle:publication` artifact compiled by the admin editor. Block
payload schema validation belongs to Publisle in the editor. The server checks
block envelopes, IDs, builtin allowlists, version, and payload size.

Changing a document invalidates its compiled artifact. For a published article,
upload the replacement document and compilation together, or unpublish first.
Compilation with error diagnostics cannot be published. The default JSON request
limit is 7 MiB, the document limit is 2 MiB, and the artifact limit is 4 MiB.

`GET /api/v1/blogs` supports `featured`, `page`, and `per_page` (maximum 100).
It returns cards plus pagination metadata. Detail responses omit canonical
editor documents and include compiled artifacts. Reads support ETag conditional
requests, gzip, and Cache-Control. `/sitemap-blogs.xml` lists published slugs.
Pagination currently occurs in memory after reading matching rows.

Public comments use `/api/v1/blogs/{slug}/comments`. Anonymous submissions are
pending until approved. OAuth comments are approved immediately, with the author
name taken from the verified identity. Bodies remain plain text, including any
literal markup. The host must render comments as text. Comment reads paginate
approved entries only. A hidden `website` input is the honeypot. Name and body
limits are 80 and 2000 characters. Optional legacy email input is validated but
not retained. Rate limits are shared across HTTP workers in one process.

The API must sit behind a trusted proxy that overwrites `X-Forwarded-For` with
the client address, and the backend port must not be publicly exposed. The last
forwarded address is used for rate limits. Multiple API replicas do not share
in-memory rate limits.

Optional OAuth providers need their client ID and secret. Register callbacks at
`{PUBLIC_BASE_URL}/api/v1/auth/google/callback` and the analogous GitHub path.
Both flows use provider-bound state and S256 PKCE. `COMMENTER_REDIRECT_URL` controls
the return location; it defaults to the blog index. Live provider exchange needs
real provider credentials and is not exercised by local tests.

Compiled HTML and island props come from an authenticated editor. Reader hosts
must sanitize HTML and use an explicit local implementation registry. Never load
module URLs from stored artifacts. Custom blocks must have a namespaced type;
register their payload schemas and approved renderers in the host applications.

Validation: `cargo fmt --check`, `cargo test --all-targets`, and
`cargo test --no-default-features --all-targets`. The publishing integration test
covers migration reruns, authentication, CSRF, draft isolation, publication,
conditional reads, sitemap, pagination, stale compilation, legacy conversion,
moderation, logout, and OAuth start/state validation. No live data is migrated
by the tests.
