# Task: Add artist URL to unsplash response (VAR-695)

The `/unsplash` endpoint currently returns a cached random nature photo from
the Unsplash API, serialized with only the image `url`, `photographer` name,
and `created_at`. We need to properly attribute the photo to its artist by
also returning a URL linking to the artist's page on Unsplash. The URL should
come from the upstream Unsplash API response, be persisted alongside the
cached photo, and be included in the endpoint's JSON response.