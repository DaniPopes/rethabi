# Changelog

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0).

## [1.0.0] - 2023-03-DD

## Unreleased

### Changed

-   `primitive_types` and `ethereum_types` to `revm_primitives`
    -   `Address`: `ethereum_types::Address -> revm_primitives::Address`, no other changes
    -   `Hash`: `ethereum_types::H256 -> revm_primitives::B256`, no other changes
    -   `Int & Uint`: `ethereum_types::U256 -> revm_primitives::U256`
        -   `<integer>.into()` -> `U256::from`
        -   `<[u8; 32]>.into()` -> `U256::from_be_bytes`
        -   `U256.into()` -> `U256::to_be_bytes`
        -   `U256::from_dec_str(&str)` -> `U256::from_str_radix(&str, 10)`
