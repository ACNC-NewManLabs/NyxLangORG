# Search and Documentation Platform

## Search

Current implementation uses registry DB queries with tag filtering and popularity ranking from download counts.

Production-scale recommendation:

- stream package metadata into OpenSearch
- maintain denormalized search documents
- support typo tolerance and weighted relevance

## Documentation

The registry serves versioned docs URLs through `/api/v1/docs/{name}/{version}`.

Production-scale recommendation:

- generate docs during verification pipeline
- publish to static object store
- front with global CDN
- index symbols and full-text docs for search
