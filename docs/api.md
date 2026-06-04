# AccelerateSearch REST API

All routes are versioned under `/api/v1/`. The OpenAPI specification is
served at `/api-docs/openapi.json` and the Swagger UI at
`/swagger-ui/`.

The response format for errors is:

```json
{
  "error": "code_snake_case",
  "message": "Human-readable description.",
  "code": 404
}
```

## System (no auth required)

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/health` | Health check |
| `GET` | `/version` | Binary version info (version, commit SHA, commit date) |
| `GET` | `/stats` | Global statistics (collection count, document count) |
| `GET` | `/metrics` | Prometheus metrics (gated by `[metrics].enabled`) |
| `GET` | `/instance-id` | Per-instance UUID |

> All other endpoints require the master key or a scoped API key in
> the `Authorization: Bearer <key>` header.

## Collections

| Method | Path | Description |
| --- | --- | --- |
| `POST`   | `/api/v1/collections` | Create collection |
| `GET`    | `/api/v1/collections` | List collections |
| `GET`    | `/api/v1/collections/{uid}` | Get collection |
| `PATCH`  | `/api/v1/collections/{uid}` | Update collection metadata |
| `DELETE` | `/api/v1/collections/{uid}` | Delete collection |
| `GET`    | `/api/v1/collections/{uid}/stats` | Collection stats |
| `GET`    | `/api/v1/collections/{uid}/settings` | Get full settings blob |
| `PATCH`  | `/api/v1/collections/{uid}/settings` | Update full settings blob |
| `DELETE` | `/api/v1/collections/{uid}/settings` | Reset settings to defaults |

### Per-setting endpoints (leaf GET / PUT / DELETE)

The following "leaf" settings each have their own GET/PUT/DELETE trio so
clients can manage individual settings without round-tripping the full
`CollectionSettings` blob:

| Method | Path |
| --- | --- |
| `GET` / `PUT` / `DELETE` | `/api/v1/collections/{uid}/settings/filterable-attributes` |
| `GET` / `PUT` / `DELETE` | `/api/v1/collections/{uid}/settings/sortable-attributes` |
| `GET` / `PUT` / `DELETE` | `/api/v1/collections/{uid}/settings/searchable-attributes` |
| `GET` / `PUT` / `DELETE` | `/api/v1/collections/{uid}/settings/displayed-attributes` |
| `GET` / `PUT` / `DELETE` | `/api/v1/collections/{uid}/settings/stop-words` |
| `GET` / `PUT` / `DELETE` | `/api/v1/collections/{uid}/settings/ranking-rules` |
| `GET` / `PUT` / `DELETE` | `/api/v1/collections/{uid}/settings/typo-tolerance` |
| `GET` / `PUT` / `DELETE` | `/api/v1/collections/{uid}/settings/distinct-field` |
| `GET` / `PUT` / `DELETE` | `/api/v1/collections/{uid}/settings/synonyms` |
| `GET` / `PATCH` / `DELETE` | `/api/v1/collections/{uid}/settings/embedders` |

> The `embedders` leaf uses `PATCH` (not `PUT`) because the embedder
> configuration is a JSON object that is deep-merged, not replaced.

## Documents

| Method | Path | Description |
| --- | --- | --- |
| `POST`   | `/api/v1/collections/{uid}/documents` | Add or replace documents (upsert by primary key) |
| `PUT`    | `/api/v1/collections/{uid}/documents` | Partial update by primary key (only supplied fields are written) |
| `GET`    | `/api/v1/collections/{uid}/documents` | List documents (paginated) |
| `GET`    | `/api/v1/collections/{uid}/documents/{id}` | Get a single document by primary key |
| `DELETE` | `/api/v1/collections/{uid}/documents/{id}` | Delete one document |
| `DELETE` | `/api/v1/collections/{uid}/documents` | Delete every document in the collection |
| `POST`   | `/api/v1/collections/{uid}/documents/delete-batch` | Bulk delete by an array of IDs |
| `GET`    | `/api/v1/collections/{uid}/documents/export?format=` | Export as `ndjson`, `json`, or `csv` |

## Search

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/api/v1/collections/{uid}/search` | Full search (POST is recommended for complex filters) |
| `GET`  | `/api/v1/collections/{uid}/search?q=&offset=&limit=&filter=&facets=` | Search (GET, lightweight) |
| `GET`  | `/api/v1/collections/{uid}/autocomplete?q=&limit=` | FST-backed term suggestions |
| `POST` | `/api/v1/multi-search` | Multi-collection search |

### Search request body

```json
{
  "q": "rust search",
  "offset": 0,
  "limit": 20,
  "filter": "rating >= 4 AND status = \"active\"",
  "facets": ["category", "brand"],
  "attributes_to_retrieve": ["id", "title", "price"],
  "attributes_to_highlight": ["title", "body"],
  "sort": ["price:asc"],
  "show_ranking_score": true,
  "hybrid": { "semantic_ratio": 0.5, "embedder": "default" },
  "vector": [0.1, 0.2, 0.3],
  "distinct": "sku"
}
```

### Search response

```json
{
  "query": "rust search",
  "hits": [
    {
      "document": { "id": "1", "title": "Rust in Action", "price": 39.95 },
      "formatted": { "title": "<em>Rust</em> in Action" },
      "ranking_score": 0.86
    }
  ],
  "offset": 0,
  "limit": 20,
  "estimatedTotalHits": 142,
  "processingTimeMs": 4,
  "facetDistribution": { "category": { "counts": { "book": 87, "video": 55 } } }
}
```

### Autocomplete response

```json
{
  "query": "rus",
  "suggestions": [
    { "term": "rust",     "total_term_freq": 1234 },
    { "term": "rusty",    "total_term_freq":  17  },
    { "term": "russian",  "total_term_freq":   9  }
  ],
  "processingTimeMs": 1
}
```

### Filter expression grammar

```
expr        := or
or          := and ( "OR" and )*
and         := not ( "AND" not )*
not         := "NOT" not | atom
atom        := "(" expr ")" | comparison
comparison  := field op value
op          := "=" | "!=" | ">" | ">=" | "<" | "<=" | "TO"
            | "IN" | "NOT" "IN" | "EXISTS" | "IS" "NULL" | "IS" "NOT" "NULL"
            | "CONTAINS" | "STARTS_WITH" | "ENDS_WITH" | "LIKE"
            | "GEO_BBOX" lat lng lat lng
            | "GEO_RADIUS" lat lng meters
value       := number | string | bool | null | array
```

`LIKE` patterns use `%` for "any" and `_` for "single character".

## Tasks

| Method | Path | Description |
| --- | --- | --- |
| `GET`    | `/api/v1/tasks` | List tasks (paginated) |
| `GET`    | `/api/v1/tasks/{taskUid}` | Get a single task by UID |
| `DELETE` | `/api/v1/tasks` | Cancel every queued/pending task |
| `POST`   | `/api/v1/tasks/cancel` | Cancel a subset of tasks by filter (uid prefix, type, status) |

## API Keys

| Method | Path | Description |
| --- | --- | --- |
| `GET`    | `/api/v1/keys` | List keys |
| `POST`   | `/api/v1/keys` | Create key |
| `GET`    | `/api/v1/keys/{key_or_uid}` | Get key by raw key value or UID |
| `PATCH`  | `/api/v1/keys/{key_or_uid}` | Update key metadata (name, scopes, expiry) |
| `DELETE` | `/api/v1/keys/{key_or_uid}` | Delete key |

## Tenant Tokens

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/api/v1/tenant-tokens` | Mint a short-lived HS256 JWT scoped to a search API key |

## Snapshots

| Method | Path | Description |
| --- | --- | --- |
| `POST`   | `/api/v1/snapshots` | Create snapshot |
| `GET`    | `/api/v1/snapshots` | List snapshots |
| `GET`    | `/api/v1/snapshots/{name}` | Get snapshot info |
| `DELETE` | `/api/v1/snapshots/{name}` | Delete snapshot |
| `POST`   | `/api/v1/snapshots/{name}/restore` | Restore snapshot |

## Indexes (alias for collections)

The `/api/v1/indexes/*` routes are aliases for `/api/v1/collections/*`
and accept the same payloads. They exist for Meilisearch compatibility
on the search-rules endpoint (which lives at
`/api/v1/indexes/{uid}/settings/rules`).

| Method | Path | Description |
| --- | --- | --- |
| `POST`   | `/api/v1/indexes` | Create index |
| `GET`    | `/api/v1/indexes` | List indexes |
| `GET`    | `/api/v1/indexes/{uid}` | Get index |
| `PATCH`  | `/api/v1/indexes/{uid}` | Update index metadata |
| `DELETE` | `/api/v1/indexes/{uid}` | Delete index |
| `GET`    | `/api/v1/indexes/{uid}/stats` | Index stats |
| `POST`   | `/api/v1/swap-indexes` | Atomically swap two indexes |

## Search Rules (curated queries)

| Method | Path | Description |
| --- | --- | --- |
| `GET`    | `/api/v1/indexes/{uid}/settings/rules` | Get ruleset |
| `POST`   | `/api/v1/indexes/{uid}/settings/rules` | Replace ruleset |
| `DELETE` | `/api/v1/indexes/{uid}/settings/rules` | Delete ruleset |

## Hooks (webhooks)

| Method | Path | Description |
| --- | --- | --- |
| `GET`    | `/api/v1/hooks` | List hooks |
| `GET`    | `/api/v1/hooks/{id}` | Get hook |
| `POST`   | `/api/v1/hooks` | Create hook |
| `PATCH`  | `/api/v1/hooks/{id}` | Update hook |
| `DELETE` | `/api/v1/hooks/{id}` | Delete hook |

## Network and experimental features

| Method | Path | Description |
| --- | --- | --- |
| `GET`   | `/api/v1/network` | Cluster network info (skeleton) |
| `GET`   | `/api/v1/experimental-features` | List feature toggles |
| `PATCH` | `/api/v1/experimental-features` | Update feature toggles |

## OpenAPI and Swagger

| Path | Description |
| --- | --- |
| `/api-docs/openapi.json` | OpenAPI 3.1 spec in JSON |
| `/swagger-ui/`           | Interactive Swagger UI (HTML) |
| `/swagger-ui/{tail:.*}`  | Swagger UI assets |

Both are gated by the `[api_docs]` TOML section. Disabling either key
returns a 404 for that path.
