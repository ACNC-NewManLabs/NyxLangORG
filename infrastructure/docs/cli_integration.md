# CLI Integration Examples

Assume registry is running at `http://127.0.0.1:8090`.

## Register Developer

```bash
curl -X POST http://127.0.0.1:8090/api/v1/auth/register \
  -H 'content-type: application/json' \
  -d '{
    "username":"alice",
    "email":"alice@example.com",
    "signing_public_key":"alice-public-key-v1"
  }'
```

Export returned API key:

```bash
export NYX_API_KEY=<returned_key>
export NYX_SIGNING_PUBLIC_KEY=alice-public-key-v1
export NYX_REGISTRY_URL=http://127.0.0.1:8090
```

## Publish

```bash
cargo run --manifest-path package_manager/nyxpkg/Cargo.toml -- publish infrastructure/schemas/nyx.toml.example
```

## Search

```bash
cargo run --manifest-path package_manager/nyxpkg/Cargo.toml -- search example
```

## Info

```bash
cargo run --manifest-path package_manager/nyxpkg/Cargo.toml -- info example_lib
```

## Install

```bash
cargo run --manifest-path package_manager/nyxpkg/Cargo.toml -- install example_lib
```
