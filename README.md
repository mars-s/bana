<h1 align="center">TanaCraft</h1>

<p align="center">
  <b>A keyboard-first outliner with supertags, fields and daily notes; an open-source, clean-room
  reimplementation of the Tana outliner, rebuilt in pure Rust.</b>
</p>

<p align="center">
  Every bullet is a node. Tag a node with a supertag and it gains that tag's fields. Reference any
  node from anywhere. Write by hierarchy or by day. Your graph is one local JSON file.
</p>

## Status

Early. See [ROADMAP.md](ROADMAP.md) and the [parity report](docs/parity.md).

## Quick start

```sh
cargo run -p tanacraft -- --sample          # desktop app with the sample graph
cargo run -p tanacraft-cli -- --help         # command line
```

## License

MIT OR Apache-2.0. Clean-room: built from public documentation and a black-box behaviour spec, with
every asset original. Not affiliated with Tana Inc.
