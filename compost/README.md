# compost/

Standalone trading engines that operate outside the Rust workspace.

## kalshi/

Rust-based Kalshi trading engine. Currently lives in `crates/pm-kalshi` as a workspace member
for development convenience. Will be moved here when it's stable enough to operate independently.

The root `Cargo.toml` already excludes `compost/kalshi` in anticipation of this migration.

## polymarket/

Python-based Polymarket weather market trader. Not yet ported to this repo.
