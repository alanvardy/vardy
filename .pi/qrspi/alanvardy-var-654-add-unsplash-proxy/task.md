# Task: Add unsplash proxy (VAR-654)

Add a `GET /unsplash` endpoint that fetches a random "nature" picture from
the Unsplash API and stores it in a new `unsplash_pictures` SQLite table.
If the stored picture is more than 6 hours old, fetch a fresh one from the
API and store it. Authentication uses an `UNSPLASH_API_KEY` environment
variable. The ticket points to the `../api` project for inspiration on how
to build it.
