# Task: Add /dump (VAR-655)

Add a `/dump/<key>` route pair to the vardy web service. `POST /dump/<key>`
accepts an arbitrary JSON body and stores it as a blob in a new `dumps` SQL
table, associated with the given key. `GET /dump/<key>` returns the list of
stored dumps for that key as JSON, identified by id.

This exists so the service can act as a simple generic JSON capture/dump
endpoint.
