# rethabi

[![Crates.io][crates-badge]][crates-url]
[![License][license-badge]][license-url]
[![CI Status][actions-badge]][actions-url]

A Solidity Contract Application Binary Interface (ABI) implementation in Rust.

Forked from [`rust-ethereum/ethabi`](https://github.com/rust-ethereum/ethabi). Original documentation and CLI reference can be found at [OLD-README](./OLD-README.md) and [OLD-CHANGELOG](./OLD-CHANGELOG.md).

## Motivation

We mostly wanted to replace `primitive-types` and `uint` from Parity with the more modern implementation `ruint` which uses const generics.

[crates-badge]: https://img.shields.io/crates/v/rethabi.svg
[crates-url]: https://crates.io/crates/rethabi
[license-badge]: https://img.shields.io/badge/license-MIT_OR_Apache--2.0-blue.svg
[license-url]: https://github.com/danipopes/rethabi/blob/master/LICENSE-MIT
[actions-badge]: https://github.com/danipopes/rethabi/workflows/CI/badge.svg
[actions-url]: https://github.com/danipopes/rethabi/actions?query=workflow%3ACI+branch%3Amaster
