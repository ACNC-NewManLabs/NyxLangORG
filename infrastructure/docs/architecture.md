# Nyx Global Infrastructure Architecture

## Global Topology

```text
Nyx Developers
      |
      v
Nyx CLI / nyxpkg
      |
      v
Global Nyx Registry API
      |
      v
Distributed Mirror Network
      |
      v
Package Storage + Build/Verification Workers
```

## Services

- `registry-server`: authoritative package index, metadata, auth, verification gating
- `collab-server`: global developer collaboration, issue-solving, contributor discovery, and shared optimization insights
- `mirror-agent`: regional mirror synchronization from snapshot API
- `build-worker`: deterministic multi-target artifact verification
- `docs host` (backed by docs URL pointers): versioned docs routing
- `search`: implemented in registry query path; production can externalize to OpenSearch

## Security Model

- API-key authenticated publish requests
- package ownership enforced per package namespace
- signing requirement (`signature` validated with registered signing key)
- verification pipeline before accept:
  - metadata + semver checks
  - dependency validation
  - security pattern scan
  - deterministic multi-target artifact hash generation

## Scalability Notes

For global-scale production:

- shard package metadata DB by package prefix
- use object storage/CDN for artifacts
- move search to dedicated index cluster
- run mirror nodes per region (US/EU/APAC/SA/AF)
- cache hot package metadata at edge
