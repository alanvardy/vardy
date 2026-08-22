# spark-api tickets

21 issues in the spark-api Linear project.

SPA-1:Add Prometheus counter for location creation: Add a Prometheus counter (app_locations_created_total) that increments every time a new location is successfully inserted via POST /location.
SPA-3:Use credit card for OpenRouter: (no description)
SPA-4:Use credit card for Fly.io: And enable postgres
SPA-5:Use credit card for GitHub actions: Only enable for API, not for mobile app
SPA-6:Come up with repeating audit prompts: audit-architecture
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
