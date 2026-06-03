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

## System

| Method | Path | Description |
| --- | --- | --- |
| `GET`  | `/health` | Health check (no auth) |
| `GET`  | `/version` | Binary version info |
| `GET`  | `/stats` | Global statistics |
| `GET`  | `/metrics` | Prometheus metrics |
| `GET`  | `/instance-id` | Per-instance UUID |

## Collections

| Method | Path | Description |
| --- | --- | --- |
| `POST`   | `/api/v1/collections` | Create collection |
| `GET`    | `/api/v1/collections` | List collections |
| `GET`    | `/api/v1/collections/{uid}` | Get collection |
| `PATCH`  | `/api/v1/collections/{uid}` | Update collection metadata |
| `DELETE` | `/api/v1/collections/{uid}` | Delete collection |
| `GET`    | `/api/v1/collections/{uid}/stats` | Collection stats |
| `GET`    | `/api/v1/collections/{uid}/settings` | Get settings |
| `PATCH`  | `/api/v1/collections/{uid}/settings` | Update settings |
| `DELETE` | `/api/v1/collections/{uid}/settings` | Reset settings |

### Per-setting endpoints

The following endpoints all follow the same pattern with the setting
name in the path:

* `GET /api/v1/collections/{uid}/settings/{setting}`
* `PUT /api/v1/collections/{uid}/settings/{setting}`
* `DELETE /api/v1/collections/{uid}/settings/{setting}`

Settings: `filterable-attributes`, `sortable-attributes`,
`searchable-attributes`, `displayed-attributes`, `stop-words`,
`ranking-rules`, `typo-tolerance`, `distinct-field`.

## Documents

| Method | Path | Description |
| --- | --- | --- |
| `POST`   | `/api/v1/collections/{uid}/documents` | Add or replace documents |
| `PUT`    | `/api/v1/collections/{uid}/documents` | Partial update by primary key |
| `GET`    | `/api/v1/collections/{uid}/documents` | List documents (paginated) |
| `GET`    | `/api/v1/collections/{uid}/documents/{id}` | Get single document |
| `DELETE` | `/api/v1/collections/{uid}/documents/{id}` | Delete one document |
| `DELETE` | `/api/v1/collections/{uid}/documents` | Delete all documents |
| `POST`   | `/api/v1/collections/{uid}/documents/delete-batch` | Bulk delete by IDs |
| `GET`    | `/api/v1/collections/{uid}/documents/export?format=` | Export as NDJSON/JSON/CSV |

## Search

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/api/v1/collections/{uid}/search` | Full search |
| `GET`  | `/api/v1/collections/{uid}/search?q=&offset=&limit=&filter=&facets=` | Search (GET) |
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
| `GET`    | `/api/v1/tasks` | List tasks |
| `GET`    | `/api/v1/tasks/{taskUid}` | Get task |
| `DELETE` | `/api/v1/tasks` | Cancel all tasks |
| `POST`   | `/api/v1/tasks/cancel` | Cancel by filter |

## API Keys

| Method | Path | Description |
| --- | --- | --- |
| `GET`    | `/api/v1/keys` | List keys |
| `POST`   | `/api/v1/keys` | Create key |
| `GET`    | `/api/v1/keys/{keyOrUid}` | Get key |
| `PATCH`  | `/api/v1/keys/{keyOrUid}` | Update key |
| `DELETE` | `/api/v1/keys/{keyOrUid}` | Delete key |

## Tenant Tokens

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/api/v1/tenant-tokens` | Mint short-lived HS256 JWT |

## Snapshots

| Method | Path | Description |
| --- | --- | --- |
| `POST`   | `/api/v1/snapshots` | Create snapshot |
| `GET`    | `/api/v1/snapshots` | List snapshots |
| `GET`    | `/api/v1/snapshots/{name}` | Get snapshot info |
| `DELETE` | `/api/v1/snapshots/{name}` | Delete snapshot |
| `POST`   | `/api/v1/snapshots/{name}/restore` | Restore snapshot |

## Synonyms

| Method | Path | Description |
| --- | --- | --- |
| `GET`    | `/api/v1/collections/{uid}/settings/synonyms` | Get synonyms |
| `PUT`    | `/api/v1/collections/{uid}/settings/synonyms` | Replace synonyms |
| `DELETE` | `/api/v1/collections/{uid}/settings/synonyms` | Delete synonyms |

## Embedders

| Method | Path | Description |
| --- | --- | --- |
| `GET`    | `/api/v1/collections/{uid}/settings/embedders` | List embedders |
| `PATCH`  | `/api/v1/collections/{uid}/settings/embedders` | Update embedders |
| `DELETE` | `/api/v1/collections/{uid}/settings/embedders` | Reset embedders |

## Indexes (alias for collections)

Same shape as collections, with a single additional endpoint:

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/api/v1/swap-indexes` | Atomically swap two indexes |

## Hooks (webhooks)

| Method | Path | Description |
| --- | --- | --- |
| `GET`    | `/api/v1/hooks` | List hooks |
| `GET`    | `/api/v1/hooks/{id}` | Get hook |
| `POST`   | `/api/v1/hooks` | Create hook |
| `PATCH`  | `/api/v1/hooks/{id}` | Update hook |
| `DELETE` | `/api/v1/hooks/{id}` | Delete hook |

## Search Rules (curated queries)

| Method | Path | Description |
| --- | --- | --- |
| `GET`    | `/api/v1/indexes/{uid}/settings/rules` | Get ruleset |
| `POST`   | `/api/v1/indexes/{uid}/settings/rules` | Replace ruleset |
| `DELETE` | `/api/v1/indexes/{uid}/settings/rules` | Delete ruleset |

## Network and experimental features

| Method | Path | Description |
| --- | --- | --- |
| `GET`   | `/api/v1/network` | Cluster network info |
| `GET`   | `/api/v1/experimental-features` | List toggles |
| `PATCH` | `/api/v1/experimental-features` | Update toggles |
