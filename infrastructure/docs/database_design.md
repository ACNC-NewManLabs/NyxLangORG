# Registry Database Design

Schema file: `infrastructure/registry-server/schema.sql`

## Core Entities

- `developers`: account identity and signing key
- `api_keys`: publish/auth credentials
- `packages`: package namespace and ownership
- `package_versions`: versioned manifests, signature, verification report
- `package_dependencies`: dependency graph edges
- `package_tags`: tags for search and categorization
- `build_artifacts`: target-specific verified artifacts
- `package_downloads`: popularity and traffic analytics
- `mirrors`: regional mirror nodes and health

## Indexing Strategy (recommended)

- unique index on `packages.name`
- index `package_versions(package_id, published_at DESC)`
- index `package_dependencies(dep_name)`
- index `package_tags(tag)`
- index `package_downloads(package_id, downloaded_at)`
