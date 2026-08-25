# Task — Add unsplash/random

Add a new endpoint `GET /unsplash/random` (VAR-708). When fewer than 5
entries exist in the `unsplash_pictures` table, it fetches a fresh picture
from the Unsplash API, stores it, and returns it; when 5 or more entries
exist, it returns one entry chosen at random. The response body must match
the existing `GET /unsplash` shape: `{ url, photographer, created_at }`.