# cargo-member

[![CI](https://github.com/qryxip/cargo-member/workflows/CI/badge.svg)](https://github.com/qryxip/cargo-member/actions?workflow=CI)
[![codecov](https://codecov.io/gh/qryxip/cargo-member/branch/master/graph/badge.svg)](https://codecov.io/gh/qryxip/cargo-member/branch/master)
[![dependency status](https://deps.rs/repo/github/qryxip/cargo-member/status.svg)](https://deps.rs/repo/github/qryxip/cargo-member)
[![Crates.io](https://img.shields.io/badge/crates.io-not%20yet-inactive)](https://crates.io)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-informational)](https://crates.io)

Cargo subcommand for managing workspace members.

## Installation

### `master`

```console
$ cargo install --git https://github.com/qryxip/cargo-member
```

## Usage

### `cargo member include`

```console
$ tree "$PWD"
/home/ryo/src/local/workspace
├── a
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── b
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── Cargo.lock
└── Cargo.toml

4 directories, 6 files
$ cat ./Cargo.toml
[workspace]
members = ["a"]
exclude = []
$ cargo member include ./b
      Adding "b" to `workspace.members`
$ cat ./Cargo.toml
[workspace]
members = ["a", "b"]
exclude = []
```

### `cargo member exclude`

```console
$ tree "$PWD"
/home/ryo/src/local/workspace
├── a
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── b
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── Cargo.lock
└── Cargo.toml

4 directories, 6 files
$ cat ./Cargo.toml
[workspace]
members = ["a", "b"]
exclude = []
$ cargo member exclude ./b # or `-p b`
    Removing "b" from `workspace.members`
      Adding "b" to `workspace.exclude`
$ cat ./Cargo.toml
[workspace]
members = ["a"]
exclude = ["b"]
```

### `cargo member focus`

```console
$ tree "$PWD"
/home/ryo/src/local/workspace
├── a
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── b
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── c
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── Cargo.lock
└── Cargo.toml

6 directories, 8 files
$ cat ./Cargo.toml
[workspace]
members = ["a", "b", "c"]
exclude = []
$ cargo member focus ./a # or `-p a`
    Removing "b" from `workspace.members`
    Removing "c" from `workspace.members`
      Adding "b" to `workspace.exclude`
      Adding "c" to `workspace.exclude`
$ cat ./Cargo.toml
[workspace]
members = ["a"]
exclude = ["b", "c"]
```

### `cargo member new`

```console
$ tree "$PWD"
/home/ryo/src/local/workspace
├── a
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── Cargo.lock
└── Cargo.toml

2 directories, 4 files
$ cargo member new b
     Created binary (application) `/home/ryo/src/local/workspace/b` package
      Adding "b" to `workspace.members`
$ cat ./Cargo.toml
[workspace]
members = ["a", "b"]
exclude = []
$ cargo metadata --format-version 1 | jq -r '.packages | map(.name) | sort[]'
a
b
```

### `cargo member cp`

```console
$ tree "$PWD"
/home/ryo/src/local/workspace
├── a
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── Cargo.lock
└── Cargo.toml

2 directories, 4 files
$ cat ./Cargo.toml
[workspace]
members = ["a"]
exclude = []
$ cargo member cp a ./b
     Copying `/home/ryo/src/local/workspace/a` to `/home/ryo/src/local/workspace/b`
info: Found workspace `/home/ryo/src/local/workspace`
      Adding "b" to `workspace.members`
$ tree "$PWD"
/home/ryo/src/local/workspace
├── a
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── b
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── Cargo.lock
└── Cargo.toml

4 directories, 6 files
$ cat ./Cargo.toml
[workspace]
members = ["a", "b"]
exclude = []
$ cargo metadata --format-version 1 | jq -r '.packages | map(.name) | sort[]'
a
b
```

### `cargo member rm`

```console
$ tree "$PWD"
/home/ryo/src/local/workspace
├── a
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── b
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── Cargo.lock
└── Cargo.toml

4 directories, 6 files
$ cat ./Cargo.toml
[workspace]
members = ["a", "b"]
exclude = []
$ cargo member rm ./b # or `-p b`
    Removing directory `/home/ryo/src/local/workspace/b`
    Removing "b" from `workspace.members`
$ tree "$PWD"
/home/ryo/src/local/workspace
├── a
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── Cargo.lock
└── Cargo.toml

2 directories, 4 files
$ cat ./Cargo.toml
[workspace]
members = ["a"]
exclude = []
```

### `cargo member mv`

```console
$ tree "$PWD"
/home/ryo/src/local/workspace
├── a
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── Cargo.lock
└── Cargo.toml

2 directories, 4 files
$ cat ./Cargo.toml
[workspace]
members = ["a"]
exclude = []
$ cargo member mv a ./b
     Copying `/home/ryo/src/local/workspace/a` to `/home/ryo/src/local/workspace/b`
info: Found workspace `/home/ryo/src/local/workspace`
      Adding "b" to `workspace.members`
    Removing directory `/home/ryo/src/local/workspace/a`
    Removing "a" from `workspace.members`
$ tree "$PWD"
/home/ryo/src/local/workspace
├── b
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── Cargo.lock
└── Cargo.toml

2 directories, 4 files
$ cat ./Cargo.toml
[workspace]
members = [ "b"]
exclude = []
$ cargo metadata --format-version 1 | jq -r '.packages[] | .name'
b
```

## License

Licensed under <code>[MIT](https://opensource.org/licenses/MIT) OR [Apache-2.0](http://www.apache.org/licenses/LICENSE-2.0)</code>.
