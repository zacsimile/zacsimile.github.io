# zacsimile.github.io

Source and generated pages for my personal website.

Edit page content and blog metadata in `sitegen/content/`, shared templates in
`sitegen/src/main.rs`, and styles in `css/main.css`. Root HTML files and `rss.xml`
are generated; edit their sources rather than these outputs.

Build from the repository root with Rust and Cargo installed:

```sh
cargo run --manifest-path sitegen/Cargo.toml
cargo test --manifest-path sitegen/Cargo.toml
cargo run --manifest-path sitegen/Cargo.toml -- --check
```

Commit both the generator sources and generated pages. GitHub Pages serves the
repository root; no workflow runs the Rust generator automatically.

See [sitegen/README.md](sitegen/README.md) for page metadata and blog instructions,
and [DESIGN.md](DESIGN.md) for local design backup and restoration details.
