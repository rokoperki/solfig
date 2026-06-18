# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-06-18

### Added
- Telemetry sidebar now shows delinquent (non-voting) validator count
  (`getVoteAccounts`) and the median recent priority fee in micro-lamports per
  compute unit (`getRecentPrioritizationFees`). Both refresh on the existing
  slow-stats tick, so the fast telemetry path is unchanged.
- Live slot heartbeat: a fast `getSlot` poll ticks the displayed slot between
  full telemetry refreshes and pulses a blip (`●`) in the sidebar on each new
  slot, decaying to a dim flatline when the chain/RPC stalls.
- Wallet labels: press `n` to name the current wallet. Labels are stored in the
  config's `address_labels` map (Solana-CLI compatible) and shown on the keypair
  field, in the keypair picker (also searchable), and on transfer recipients.
- Transfer screen shows the sender balance and validates the amount against it,
  both while typing and on review, so a transfer that would exceed the balance
  can't be submitted.

## [0.1.1] - 2026-06-18

Maintenance release — no user-facing behavior changes.

### Changed
- Migrated YAML handling from the unmaintained `serde_yaml` to the maintained
  fork `serde_yaml_ng`. Config files are read and written identically and remain
  fully compatible with the Solana CLI's `config.yml`.

### Added
- GitHub Actions CI: `fmt`, `clippy -D warnings`, and the full test suite on
  every push and pull request.
- A `cargo audit` CI job (with a weekly schedule) to catch dependency
  advisories automatically.
- README badges (CI, crates.io, license) and a documented pre-push checklist.

### Security
- Resolves the `serde_yaml` unmaintained advisory; `cargo audit` reports no
  vulnerabilities.

## [0.1.0] - 2026-06-18

Initial release.

### Added
- Terminal UI for editing the Solana CLI config (`~/.config/solana/cli/config.yml`),
  with an atomic save and an unsaved-changes guard.
- Cluster switching across built-in monikers (mainnet-beta, devnet, testnet,
  localhost) plus user-saved custom RPC endpoints.
- Keypair picker with fuzzy filtering across `~/.config/solana`, `~/Downloads`,
  and the current directory; generate a fresh keypair in place (written `0600`).
- Wallet features: derived pubkey and balance display, airdrops (non-mainnet),
  and SOL transfers (signing delegated to the `solana` CLI).
- Live cluster telemetry sidebar: slot, block height, epoch progress + ETA, TPS,
  validator count, transaction count, circulating supply, SOL/USD price, and RPC
  ping/version, cached per-cluster for instant switching.
- Profiles for saving and switching whole config environments.
- Web faucet launcher (opens in the browser, copies the pubkey).
- Theming: 7 built-in palettes plus user-defined themes, with live preview.
- Safety: mainnet hazard banner, copy-field and open-in-Explorer helpers.

[Unreleased]: https://github.com/rokoperki/solfig/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/rokoperki/solfig/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/rokoperki/solfig/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/rokoperki/solfig/releases/tag/v0.1.0
