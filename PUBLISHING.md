# Publishing API

Run `cargo migrate` before starting the upgraded API. The migration adds document
columns with defaults, comments, identities, and sessions without dropping existing
tables. `cargo migrate -- --seed` also inserts seed drafts when empty. Migration
can be rerun safely. Back up the database before deploying schema changes.

Configure `ADMIN_PASSWORD_HASH` with an Argon2 PHC hash for deployment.
`ADMIN_PASSWORD` is a local development fallback. `PUBLIC_BASE_URL` is the public
site origin, not the container address. Set a private `COMMENT_IP_SALT`.
Use explicit `CORS_ORIGINS` for credentialed development requests.

Admin routes live at `/api/v1/admin`. Login at `POST /session` with
`{"password":"..."}`. Subsequent requests send the session cookie. Unsafe admin
requests must send `X-CSRF-Token` equal to the readable `voidreg_admin_csrf` cookie.
`GET /session` restores a session; `DELETE /session` revokes it. Admin responses
carry `Cache-Control: no-store`. HTTPS public origins enable Secure cookies.

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
