# spark-api tickets

191 issues in the spark-api Linear project, including archived ones.

SPA-1:Add Prometheus counter for location creation: Add a Prometheus counter (app_locations_created_total) that increments every time a new location is successfully inserted via POST /location.
SPA-3:Use credit card for OpenRouter: (no description)
SPA-4:Use credit card for Fly.io: And enable postgres
SPA-5:Use credit card for GitHub actions: Only enable for API, not for mobile app
SPA-6:Come up with repeating audit prompts: audit-architecture
SPA-8:Add metrics for encounters job: We want to increment a counter every time an encounter is inserted into the database.
SPA-9:Go investigate worktree crispi: Might be able to add last 2 steps
SPA-10:Rename api to spark-api: Rename repo
SPA-11:Get new credit card from Emily: (no description)
SPA-13:Add API monitoring to updown.io: Set up uptime monitoring for the API on updown.io:
SPA-14:Purchase a domain: (no description)
SPA-15:Use AWS credentials for email: Currently using tigris
SPA-16:Set up an SES email address: The verified sender/domain email address. Parsed in Env::init() via get_string_env. The SES region and credentials come from the shared SdkConfig (already loaded fromAWS_REGION, AWS_ACCESS_KEY_ID, etc…
SPA-17:Request AWS SES production access: SES accounts start in a sandbox with severe restrictions:
SPA-18:Re-add Sign in with Apple entitlements for prod builds: The full Sign in with Apple pipeline is implemented (VAR-507): SignInWithAppleButton in AuthView, appleSignIn API endpoint, credential state check on launch. The spark.entitlements file and CODE_SIGN_…
SPA-19:Add Redis-backed email outbox for durable sending: The current email implementation (inline SES sending from handlers) was chosen as the simplest path to launch. It works for low-volume transactional email but has known limitations:
SPA-21:Retry emails: (no description)
SPA-22:Password reset and forgotten passwords: (no description)
SPA-23:Email verification: (no description)
SPA-24:Add pagination to users web page: (no description)
SPA-25:SES and Email: (no description)
SPA-26:Come up with a load testing plan: (no description)
SPA-27:Manual Postgres setup: local macOS and Fly.io provisioning: 1. **Install Postgres** (if not already installed):
SPA-28:Role plumbing + admin-only GET /users: Goal: Full vertical slice: role flows from DB → JWT → middleware → handler guard → HTTP response. Prove the architecture works with one endpoint.
SPA-29:[Critical] User delete has no cascade — orphans file rows and S3 objects: Deleting a user via DELETE /users/{id} only removes the user row. It does not cascade to the user's files — leaving orphaned file rows in the database and orphaned objects in S3.
SPA-30:[Critical] User CRUD endpoints are completely unauthenticated: The user CRUD endpoints have no authentication middleware applied. Anyone can create, read, update, or delete any user without credentials.
SPA-31:[Critical] Manual compensating transaction in image upload handler — no atomicity: The image upload handler implements a manual compensating transaction: upload to S3, then insert into DB, and if the DB insert fails, try to delete the S3 object.
SPA-32:Figure out how to run integration tests sandboxed: Right now we have to truncate the test db after every round of tests
SPA-33:RUST-6 - panic: error.html template must exist: TemplateNotFound: Sentry error alert. TemplateNotFound: template "error.html" does not exist.
SPA-34:POST /auth/apple — Sign in with Apple endpoint: The Spark iOS app is adding Sign in with Apple. The client will send the Apple identity token (JWT) to the backend for verification and user lookup/creation. The client implementation is underway (VAR…
SPA-36:Email infrastructure — trait, impls, errors, env, wiring: Phase 1 of email sending: Add infra::email with trait-based Emailer backed by AWS SES v2, wired through AppState. Includes EmailError, SES_FROM_ADDRESS env var, FakeEmailer test double, and welcome em…
SPA-37:Slice 1: Metrics module + /metrics endpoint: *Slice 1: Metrics module + /metrics endpoint**
SPA-38:Slice 2: Background heartbeat spawn: *Slice 2: Background heartbeat spawn**
SPA-39:Self-or-admin guards on GET/PUT/DELETE /users/{id}: Goal: Add role-based authorization to remaining user CRUD endpoints.
SPA-40:Docs & Validation: Phase 9 of VAR-433:
SPA-41:Dependencies & Schema Migrations: Phase 1 of VAR-433:
SPA-42:Wiring: AppState, Env, routes, main.rs: Phase 6 of VAR-433:
SPA-43:User functions: signup, login, find_by_email: Phase 5 of VAR-433:
SPA-44:Auth handlers: signup/login/refresh/logout: Phase 7 of VAR-433:
SPA-45:Error variant & Domain types: Phases 2+3 of VAR-433:
SPA-46:Tests for auth system: Phase 8 of VAR-433:
SPA-47:Auth functions (hash, JWT, require_jwt) + sessions.rs: Phase 4 of VAR-433:
SPA-48:[High] No rate limiting on any endpoint: No rate limit middleware is applied anywhere in the application. An attacker can hammer any endpoint (including the unauthenticated /feature_flags and /users routes) at will.
SPA-49:[High] Database URL hardcoded to sqlite:test.db — not configurable: The database path is hardcoded and uses a misleading filename.
SPA-50:[High] No request body size limit on image upload: The image upload handler accepts a JSON body with base64-encoded image data, but no request body size limit is configured.
SPA-51:[High] All domain types in single flat models.rs — no bounded contexts: All domain types live in a single flat file with no module boundaries. Three clearly separable domains (users, feature flags, files/images) are collapsed into one namespace.
SPA-53:Investigate whether fly.io supports rollbacks: Investigate fly.io's rollback capabilities and determine how we can use them for our deployment workflow.
SPA-54:Admin bypass on file endpoints: Goal: Allow admins to manage any user's files.
SPA-55:Add JSON body assertions to reqwest-based integration handler tests: The assertion helpers in src/test/assertions.rs take axum::Response, so they can't directly apply to the reqwest-based integration tests in the handler files. These tests currently have body-blind pat…
SPA-56:Adopt Ecto-style changeset/validation pattern with validator crate: Introduce a changeset pattern (similar to Elixir Ecto changesets) for validating input before persistence with SQLx.
SPA-57:[Medium] Handlers organized by delivery mechanism, not by domain: Handlers are organized by delivery mechanism (JSON vs HTML), not by domain:
SPA-58:[Medium] Naming inconsistency: images vs files across codebase: The codebase is inconsistent about whether it's an "images" or "files" service:
SPA-59:[Medium] S3 Client created per operation — should be cached: The S3 Client is constructed fresh on every operation. Every upload, presign_download, and delete call in src/app/files.rs creates a new aws_sdk_s3::Client from SdkConfig.
SPA-60:[Medium] ContentType::from String panics on unknown input: ContentType has a From<String> impl that panics on unknown input, violating Rust conventions (From should be infallible).
SPA-61:[Medium] Inline HTML template in error.rs — presentation in error module: The error.rs module contains render_error_page() — a 30+ line function with an inline HTML template string. This is a presentation concern leaking into the error/infrastructure module.
SPA-62:[Medium] content_type accepts arbitrary strings — no validation, XSS risk: The content_type field in UploadImage is an arbitrary String with no validation against the ContentType enum.
SPA-63:Abstract jobs: We need a separate module for each job, and tests for each job.c
SPA-64:Update ROUTES.md for authorization rules: Goal: Update API documentation to reflect new authorization rules.
SPA-65:[Low] Unnecessarily pub functions in routes.rs: Four sub-router functions in src/routes.rs are unnecessarily pub:
SPA-66:[Low] cfg!(test) guards in production files.rs — couple prod code to test config: src/app/files.rs uses cfg!(test) to skip real S3 calls during tests:
SPA-67:[Low] Missing value objects — no type-safe IDs for UserId, Email, etc.: Several primitives are used where newtype value objects would add type safety:
SPA-68:[Low] Presigned URLs generated sequentially in list endpoints: File listing endpoints generate presigned URLs sequentially in a loop — for N files, this makes N sequential calls to S3.
SPA-69:[Low] AppError::Storage too generic — all S3 failures collapse to 500: AppError::Storage collapses all S3 failures (invalid presign duration, expired credentials, missing objects, network errors) into a single opaque 500: {"error": "internal server error"}.
SPA-72:PR naming: When a PR is created, it's called the design document because that's all I've done so far, but I want it to be for the entire future.
SPA-73:Fix empty password hash: !Screenshot
SPA-74:Don’t show user passwords in logs: The LoginRequest struct at src/domain/users.rs:95-99 lacks a manual Debug impl, so password appears in logs. Fix this issue
SPA-75:Add caching for fetch_jwks: We need to repeatedly grab the same token from Apple over and over again and it does not rotate often
SPA-76:Research and design users web view: Decompose VAR-503 into research questions and explore codebase patterns for adding an admin web view for user management.
SPA-81:Remove pub: files::create should not be a public function
SPA-82:Move datetime: Should be in domain context
SPA-83:Disable sentry when running locally: (no description)
SPA-84:Turn skills into prompts: Read up on the difference between skills and prompts and make sure that I have assigned them in the correct places.
SPA-85:Authorization: Add user / admin role-based authorization to the API. Roles are stored in the users table, embedded in JWT claims, extracted by the existing require_jwt middleware into request extensions, and enforce…
SPA-86:Move more business logic into domain: (no description)
SPA-87:Rework architecture: In Domain-Driven Design, the main distinction is:
SPA-88:Add authorization to images endpoints: Bearer and a token set by env
SPA-89:CONTENT_TYPE enum: Validate before database insertion
SPA-90:Use templating engine for web pages: (no description)
SPA-91:Create dashboard for reviewing images: [X] Password protect like flags
SPA-92:Add flag columns to files table: ai_flagged_at: DateTime
SPA-93:Flag images for nudity or children’s faces: (no description)
SPA-94:Spend some time thinking about overall structure of the code base: (no description)
SPA-95:Add sentry: Need to be able to see production errors
SPA-96:See if sqlx queries can be composable: If they are not composable, then we have to leave the queries in the handlers
SPA-97:Test image routes: [X] Need to be able to mock upload and download in test
SPA-98:Keep rust tool chain up-to-date: See if github actions is keeping rust tool chain up-to-date
SPA-99:Only allow enabling and disabling a feature flag: We don't need people to be able to change the name of a feature flag because it
SPA-100:Change implementation prompt: Have agent do the curl commands itself for manual testing
SPA-101:Generate encounters: create an encounters table
SPA-104:addworktree needs to copy over database as well: Currently just copies over .env
SPA-105:Add encounters web: Just like showing locations on users, show encounters including a little map
SPA-106:Commit changes at the end of plan: When the the /5_plan prompt is finished, commit the changes so that they can be reviewed on github
SPA-109:Switch to Postgres: We are moving from sqlite to Postgres on Fly.io. No data needs to be retained or migrated.
SPA-110:Users web page has visual bug: !image.png
SPA-111:Edit script for new worktree: Need to copy over env
SPA-112:For some reason /scripts/test.sh doesn't fail if the database doesnt exist: (no description)
SPA-114:Create a web view for users: Is protected with admin username and password like the other webviews
SPA-116:Create ROUTES.md: (no description)
SPA-118:All sql queries must be validated where possible: Add a check for them
SPA-119:Set codecov patch coverage: (no description)
SPA-120:Enforce 100% test coverage in handlers: (no description)
SPA-122:Use UUID for ids: (no description)
SPA-123:Set updated_at on update functions: (no description)
SPA-124:test.rs should be mod.rs: (no description)
SPA-127:Enforce domains with arkitect: (no description)
SPA-128:Move arkitect code to separate test file: (no description)
SPA-129:Extract domain logic: (no description)
SPA-130:Dry up test code: Have some create code for test cases
SPA-132:Update to Postgres 17 on CI: (no description)
SPA-133:Create linear skill: How to create linear issues
SPA-136:Review after implementation: Have implement review after implementation
SPA-137:Maximum files per user: (no description)
SPA-138:Check if user exists before uploading file: (no description)
SPA-139:Still getting sentry panics: (no description)
SPA-140:Add columns to users table: created_at DateTime
SPA-141:Go through environment variables: [X] Make sure they are well documented in env
SPA-142:Make a web/feature_flags and web/images: Both module and endpoints
SPA-143:Planning: (no description)
SPA-144:Switch to using openrouter for LLMs: (no description)
SPA-145:Validate user_id: We need to get the user_id from the sessions table, not the url, when uploading files
SPA-146:Turn repo private: (no description)
SPA-147:Upload files to Tigris: (no description)
SPA-148:Move pi artifacts back to github: (no description)
SPA-149:Configure ticket creation to use tracer rounds: Research tracer round methods
SPA-150:Move rate limiting code out of main.rs: (no description)
SPA-151:Solve test warnings: (no description)
SPA-152:DELETE user/id/images/id: (no description)
SPA-153:Maybe dedup test suite run: Do I need to do coverage and tests as a separate action?
SPA-154:Make a skill to get address comments on a pr: (no description)
SPA-155:Email sending: We want an inexpensive email sending service, probably SES. We will use this to send updates, password resets, signup confirmation links, etc.
SPA-156:Job processing: *Implementation Handoff: Prometheus Metrics Endpoint + Background Heartbeat**
SPA-157:POST user/id/images: Uploads an image
SPA-158:GET user/id/images: List images for user
SPA-159:Manually test and document auth: How to create an account
SPA-160:Delete file entry if there is an upload error: (no description)
SPA-161:Return image URLs: (no description)
SPA-162:Create files table: id, user_id, content_type, key
SPA-163:Split up handlers: (no description)
SPA-164:Refactor environment variables: Pass them as a struct to the handlers
SPA-165:Prevent CI from running multiple times on push: (no description)
SPA-166:Cleanup modules: (no description)
SPA-167:Test feature flags query: (no description)
SPA-168:Remove uses of unwrap: We want to return proper HTTP errors instead
SPA-169:Allow creating and editing feature flags through web page: (no description)
SPA-170:Add password protection to web page: (no description)
SPA-171:Add page for viewing feature flags: (no description)
SPA-172:User/Password Auth: > **Revision 3** — removes legacy shared Bearer token; JWT-only auth on all protected routes.
SPA-173:Add model and query: (no description)
SPA-174:Make a test bucket and save credentials: (no description)
SPA-175:Add migration: (no description)
SPA-176:Add image storage: We want to use Tigris Object Storage https://fly.io/docs/tigris/
SPA-177:Feature Flags: We need to add feature flags to the API.
SPA-178:Add flagged info to list images for user: (no description)
SPA-179:Database migration for recurring jobs: This project is a Rust/Axum API with SQLite (via sqlx) deployed on Fly.io. We need a recurring jobs system.
SPA-180:Implement hourly heartbeat recurring job: This project is a Rust/Axum API with SQLite (via sqlx) deployed on Fly.io. Dependencies:
SPA-181:Prevent Fly server hibernation for recurring jobs: This project is deployed on Fly.io. The current fly.toml has auto_stop_machines = 'stop' and min_machines_running = 0, which means the server hibernates when there's no traffic. For recurring jobs to …
SPA-182:Integrate job scheduling library: This project is a Rust/Axum API with SQLite (via sqlx) deployed on Fly.io. We need to integrate a job processing library that can schedule recurring jobs using cron expressions.
SPA-183:Add ON DELETE RESTRICT to files foreign key: The files table foreign key user_id TEXT NOT NULL REFERENCES users(id) currently has no explicit ON DELETE action, which defaults to NO ACTION in SQLite (effectively same as RESTRICT for deferred FKs,…
SPA-184:[High] No repository abstraction — handlers call sqlx directly: Handlers call sqlx::query_as! / sqlx::query! directly on state.db. There is no repository trait or abstraction.
SPA-185:[High] Migration adds NOT NULL columns without DEFAULT — fails on existing data: The migration adds NOT NULL columns without a DEFAULT value, which would fail if run against a database with existing rows.
SPA-186:[High] No service layer — handlers are transaction scripts: Handlers are transaction scripts doing HTTP parsing, validation, S3 operations, database queries, and response mapping all in one function. There is no intermediate service layer.
SPA-187:[High] No data-access layer — SQL duplicated across handlers: Every handler file contains raw SQL via sqlx::query_as! / sqlx::query! macros directly against state.db. There is no shared data-access function.
SPA-188:[High] No aggregate roots — entities independently mutable, no consistency boundaries: There are no aggregate root concepts. Every entity is independently mutable with no consistency boundaries.
SPA-189:Support --json output for interactive commands (process, prioritize, remind, timebox, label, schedule, deadline): Interactive commands like list process, list prioritize, list remind, list timebox, list label, list schedule, and list deadline currently always prompt the user interactively via inquire. With the --…
SPA-190:[Medium] Sentry send_default_pii: true — verify privacy compliance: Sentry is configured with send_default_pii: true which sends user IPs and potentially sensitive headers to Sentry.
SPA-191:[Medium] No application-level info logs for mutations: There are no application-level info! / warn! log calls for business-level mutations. The tower-http layer covers request/response logging, but there are no audit events for:
SPA-192:[Medium] User update handler doesn't set updated_at — returns stale timestamp: The user update handler does not set updated_at, so every update returns the **old** timestamp.
SPA-193:[Medium] Env.aws_config cloned per request — should use Arc: Env.aws_config is an SdkConfig that gets cloned on every AppState::clone() (which happens per request). SdkConfig contains credential providers, region resolvers, etc. — cloning it repeatedly is waste…
SPA-194:[Medium] Missing tests for user update and delete: Integration tests exist for user create, get, get-by-id, and list-all. But there are no tests for:
SPA-195:Test ticket from pi coding agent: This is a test ticket to verify the linear create command works with the new assignee flag.
SPA-196:[Low] No down migrations provided: The migrations directory has 5 SQL migration files but no corresponding down/rollback migrations. This is common with sqlx but should be documented.
SPA-197:[Low] Inconsistent content_type handling between web and JSON handlers: The web images handler uses raw sqlx::query with row.get::<&str, _>("content_type") to read content_type as a plain string, while the JSON image handler uses query_as!(File, ...) which maps to the Con…
SPA-198:Mobile responsive webviews: (no description)
SPA-199:Move jobs into their own context: (no description)
SPA-200:Check image on upload: Set ai_flagged_at if nudity or children's faces
SPA-201:Add is_image function to File: Checks based om content type whether it is an image
SPA-202:Build deepseek api integration: Make it generic so we can switch it out
SPA-203:Create multipart uploads: (no description)
SPA-204:Put models and queries together: Just like blitz PG before
SPA-205:Instrument tigris uploads: Need a count of successes
SPA-206:File backups: What options do we have and are backups necessary
SPA-208:Have china mode switch websearch: (no description)
SPA-209:Validate email: (no description)
SPA-210:Show a map for locations: (no description)
SPA-211:Fix formatting: main.rs keeps formatting when I run the test suite
SPA-212:Add unique index to email: (no description)
SPA-213:Put BRIN index on location `created_at`: This is what will be used to search for connections
SPA-214:Recurring jobs: Store job in database
SPA-215:Find out if edition is deprecated in project: Saw an error message
SPA-216:Get tigris CLI and add instructions to README: (no description)
SPA-217:Test: (no description)
