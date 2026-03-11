# Forge 🔥

[![CI](https://github.com/Industrial/forge/actions/workflows/ci.yml/badge.svg)](https://github.com/Industrial/forge/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/forge.svg)](https://crates.io/crates/forge)
[![docs.rs](https://img.shields.io/docsrs/forge)](https://docs.rs/forge)
[![License: CC BY-SA 4.0](https://img.shields.io/badge/License-CC%20BY--SA%204.0-green.svg)](https://creativecommons.org/licenses/by-sa/4.0/)

## The First Batteries-Included Rust Framework

**Rails ergonomics. Rust performance. Zero compromises.**

Forge is the first full-stack Rust framework that gives you everything: auth, database, real-time, background jobs, rate limiting, caching, and more—all wired together by convention. No assembly required. No Node.js in the critical path. Just pure Rust, from zero to shipped.

```bash
curl -fsSL https://raw.githubusercontent.com/Industrial/forge/main/install.sh | sh
forge new myapp && cd myapp && forge dev
# → http://localhost:3000
```

## Why Forge Exists

**The Problem:** Rust web frameworks are powerful but incomplete. You spend weeks gluing together auth, sessions, rate limiting, background jobs, WebSockets, and real-time sync. By the time you're shipping, you've built half a framework yourself.

**The Solution:** Forge ships with everything wired. Auth? Built-in. Real-time Live Query system? Built-in. Background jobs with SQLite (no Redis needed)? Built-in. Multi-tenant isolation with Ghost Mode? Built-in. Security headers, audit logging, rate limiting, caching? All built-in.

**The Trade:** You get Rails/Django-level productivity with Rust's performance and type safety. The ecosystem is smaller than Ruby/Python's, but you need fewer packages because everything's already included.

## What Makes Forge Different

### 🚀 **Live Query: Real-Time Without the Pain**

Most frameworks make you manually manage WebSocket channels, subscriptions, and broadcasts. Forge's **Live Query** system does it automatically:

```rust
// Server: Broadcast when data changes
live::broadcast("org:123", "users", &updated_user).await?;

// Client: Automatically subscribed based on session permissions
// UI updates instantly, no polling, no manual channel management
```

Clients subscribe to scoped channels (per-org, per-resource-type). The server derives subscriptions from session permissions. When you broadcast, every subscribed connection gets the update. Single process uses in-memory pub/sub; multi-instance can swap in Redis. **UIs stay in sync without you writing WebSocket routing code.**

### 👻 **Ghost Mode: Multi-Tenant Security by Default**

Wrong-tenant data reads return 404, not empty arrays. Authorization is "shallow gate + deep scope": handler-level guards (`Action`, `Role`) plus DB-level scoping so tenant data is isolated. Roles are per-organization, not global. **You can't accidentally leak data between tenants.**

### ⚡ **One Process, Zero Redis**

Background jobs run on SQLite by default. No Redis required for development or small deployments. Run workers in-process or as a separate process. When you need Redis for multi-instance Live Query, swap it in. **Start simple, scale when needed.**

### 🎯 **Convention Over Configuration**

Strict project layout (`config/`, migrations, routes) so tooling knows where everything lives. Config is the single source of truth—no CLI flags for ports or env. Figment-based layering for env-specific overrides. **Less decision fatigue, more shipping.**

## The Full Stack

- **🔐 Auth & Authorization**: Sessions, password hashing (Argon2id), API tokens (hashed), per-org roles, Ghost Mode tenant isolation
- **💾 Database**: SeaORM + SQLite by default, migrations, seeds, connection pooling
- **⚡ Real-Time**: WebSockets, SSE, Live Query with automatic subscription management
- **📦 Background Jobs**: Apalis-based task queue, SQLite storage, in-process or separate workers
- **🛡️ Security**: Security headers (CSP, HSTS) applied by default
- **📊 Observability**: Health/live/ready endpoints, optional OpenTelemetry tracing
- **🚦 Rate Limiting**: Governor-based, per-user or per-tenant throttling
- **💨 Caching**: Application cache + optional HTTP response cache (Moka-backed)
- **🌍 i18n**: Locale and translation hooks
- **🎨 Frontend**: Vite SPA (React/Vue/Svelte) with HMR in dev, static assets in production
- **🚢 Deploy**: Shuttle (Rust hosting) + Turso (hosted SQLite), one service, zero Dockerfile

**Rust all the way:** Axum, Tokio, Tower. No Node.js in the critical path. The default template includes a Vite SPA, but you can skip the frontend entirely for API-only apps.

## How It Compares

| Feature | Next.js | Rails/Django | Axum/Actix | **Forge** |
|---------|---------|--------------|------------|-----------|
| Auth | ❌ Choose library | ✅ Built-in | ❌ DIY | ✅ **Built-in** |
| Real-time | ⚠️ Partial | ⚠️ Partial | ❌ DIY | ✅ **Live Query** |
| Background Jobs | ❌ External | ✅ Built-in | ❌ DIY | ✅ **Built-in** |
| Multi-tenant | ❌ DIY | ⚠️ Partial | ❌ DIY | ✅ **Ghost Mode** |
| Rate Limiting | ❌ DIY | ⚠️ Partial | ❌ DIY | ✅ **Built-in** |
| Type Safety | ⚠️ TypeScript | ❌ Dynamic | ✅ Rust | ✅ **Rust** |
| Performance | ⚠️ Good | ❌ Slower | ✅ Excellent | ✅ **Excellent** |
| Batteries Included | ❌ | ✅ | ❌ | ✅ **Yes** |

**Next.js** gives you one language but you still assemble auth, DB, jobs, and real-time piece by piece. **Rails/Django** have everything but lack Rust's performance and type safety. **Raw Rust frameworks** are fast but leave you building infrastructure. **Forge** gives you Rails-level productivity with Rust's performance, all in one coherent stack.

## Status: Early & Shaping the Future

Forge is **new and actively evolving**. We're building in the open: not every edge is polished, and the API may change. If you want a stable, "boring" framework, Rails or Django are safer today. **If you want to shape the future of Rust web development and get Rails ergonomics with Rust performance, Forge is built for you.**

## Quick Start

Install the CLI (no Rust toolchain required), then create and run an app.

**One-liner install (recommended)**

```bash
curl -fsSL https://raw.githubusercontent.com/Industrial/forge/main/install.sh | sh

forge new myapp
cd myapp
forge dev
```

Then open http://localhost:3000. For production: build frontend and run `forge serve`.

*Once the forge.sh domain is configured, use: `curl -fsSL https://forge.sh/install | sh`*

**Other install options**

- **Pin version:** `FORGE_VERSION=v1.2.3 curl -fsSL .../install.sh | sh`
- **With Rust:** `cargo install forge-cli` (when on crates.io) or `cargo install --git https://github.com/Industrial/forge forge-cli --bin forge`
- **From clone:** `git clone https://github.com/Industrial/forge.git && cd forge && cargo run -p forge-cli -- new myapp`

## Documentation

Details live in the `docs/` folder: CLI and project layout, config, database and SeaORM, migrations, authentication, authorization, audit logging, health and observability, rate limiting, validation, background jobs, API token auth, security, WebSockets and real-time, i18n, caching, and deploy (Shuttle + Turso). Live Query design (channels, permissions, broadcast, swappable backend) is in the multi-crate and live-channels docs.

## Contributing

We're **open source** and **community-first**. Contributions are welcome: code, docs, issues, and ideas. Check open issues, comment on design discussions, or open a PR. Be respectful and constructive; we'll do the same.

1. Fork the repo, create a branch, make your changes.
2. Run tests and linters (see [Development](#development) below).
3. Open a PR with a clear description and link any related issues.

By contributing, you agree that your contributions will be licensed under the same license as the project (see [License](#license)).

## Contributors

Thanks to everyone who has contributed to Forge:

[![Contributors](https://contrib.rocks/image?repo=Industrial/forge)](https://github.com/Industrial/forge/graphs/contributors)

*(Image generated by [contrib.rocks](https://contrib.rocks).)*

[![Star History Chart](https://api.star-history.com/svg?repos=Industrial/forge&type=Date)](https://star-history.com/#Industrial/forge)

## Development

- **Rust**: 2024 edition, format with `cargo fmt`, lint with `cargo clippy`.
- **Nix / devenv**: Use `devenv shell` for the intended environment; run commands inside it (e.g. `devenv shell -- cargo test`).
- **Quality**: Tests (including e2e), `cargo-deny` for audits, and CI on every push.

See the repo root and `.cursor/rules` for formatting, testing, and workflow details.

## License

This project is licensed under the **Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0)**. You may share and adapt the material for any purpose, including commercially, as long as you give appropriate credit and distribute your contributions under the same license. See [LICENSE](LICENSE) and [Creative Commons BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) for the full text.

The Rust crates in this repository also offer dual licensing under **MIT OR Apache-2.0** where noted in their `Cargo.toml`; for maximum permissibility in dependency use, you may use the code under those terms when applicable.

## Roadmap

### Tier 1 — Immediate DX Multipliers (Highest Impact)

1. **Zero-Friction Install (Binary Releases + One-Liner)**  
   Publish versioned binaries for macOS/Linux/Windows and support:
   ```bash
   curl -fsSL https://forge.sh/install | sh
   ```
   `cargo install --git` adds friction. A first impression should feel like Rails or Bun — instant.  
   **Impact:** Removes Rust toolchain friction from evaluators.

2. **"Golden Path" 5-Minute Tutorial**  
   A guided, opinionated walkthrough that:
   - Creates app
   - Adds model
   - Adds background job
   - Adds live update
   - Deploys  
   Make it impossible to get lost.  
   **Impact:** Reduces abandonment during evaluation.

3. **Interactive `forge doctor`**  
   Diagnostics command for:
   - Rust version
   - DB connectivity
   - Config validation
   - Missing migrations
   - Port conflicts
   - Env sanity  
   **Impact:** Converts frustration into actionable fixes.

4. **`forge check` (Static Project Validator)**  
   Pre-runtime validation:
   - Routes registered?
   - Guards mismatched?
   - Missing policies?
   - Unused migrations?
   - Broken live channels?  
   Like `cargo check`, but Forge-aware.

5. **Error Pages That Teach**  
   Instead of a bare 500, show e.g.:
   - *Missing org scope.* You called `require_role(Admin)` but no org context exists.
   - Include: what happened, why, a fix example, and a link to the docs section.  
   This is what made Ruby on Rails beloved.

### Tier 2 — Friction Killers

6. **First-Class Type-Safe Forms**  
   Generate: DTO, validation, handler, and frontend form scaffold.  
   Form handling is 40% of CRUD friction.

7. **Declarative Policy DSL**  
   Instead of writing Rust guards manually:
   ```rust
   policy! {
       Post {
           read: Member,
           write: Admin,
       }
   }
   ```
   Generate guards + DB scoping automatically.
