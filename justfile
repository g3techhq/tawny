set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

default:
    @just --list

setup:
    lefthook install

format:
    cargo fmt --all
    npm run format:web

format-check:
    cargo fmt --all -- --check
    npm run format:web:check

check: check-web check-server check-mobile check-desktop

check-web:
    cargo check

check-server:
    cargo check --no-default-features --features server

check-mobile:
    cargo check --no-default-features --features mobile

check-desktop:
    cargo check --no-default-features --features desktop

lint:
    cargo clippy --all-targets --no-deps
    cargo clippy --all-targets --no-default-features --features server --no-deps
    npm run lint:web

lint-strict:
    cargo clippy --all-targets --no-deps -- -D warnings
    cargo clippy --all-targets --no-default-features --features server --no-deps -- -D warnings
    npm run lint:web

test-node:
    npm run test:unit

test-rust:
    cargo nextest run
    cargo nextest run --no-default-features --features server

test: test-node test-rust

test-ui:
    npm run test:ui

spell:
    typos

security:
    cargo deny check

pre-push: format-check check lint test spell

quality: pre-push

ci: quality security
