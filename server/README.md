# crusty-metallian.net Server

The Rust/Actix Web server backing [crusty-metallian.net](https://crusty-metallian.net), built with Actix Web, Tera, and Datastar.

## Running Locally

```bash
cargo run
```

By default, the server loads `configuration/base.toml` layered with `configuration/local.toml` (binding to `127.0.0.1:8000`). Set `APP_ENVIRONMENT=production` for `configuration/production.toml`, and override individual settings with `APP_`-prefixed environment variables using `__` as the separator (e.g. `APP_APPLICATION__PORT=5001`).

## Testing

```bash
cargo test
```
