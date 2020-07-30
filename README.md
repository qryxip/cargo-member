# cargo-member

[![CI](https://github.com/qryxip/cargo-member/workflows/CI/badge.svg)](https://github.com/qryxip/cargo-member/actions?workflow=CI)
[![codecov](https://codecov.io/gh/qryxip/cargo-member/branch/master/graph/badge.svg)](https://codecov.io/gh/qryxip/cargo-member/branch/master)
[![Crates.io](https://img.shields.io/crates/v/cargo-member.svg)](https://crates.io/crates/cargo-member)
[![License](https://img.shields.io/crates/l/cargo-member.svg)](https://crates.io/crates/cargo-member)

Cargo subcommand for managing workspace members.

## Installation

### crates.io

```console
$ cargo install cargo-member
```

### `master`

```console
$ cargo install --git https://github.com/qryxip/cargo-member
```

### GitHub Releases

<https://github.com/qryxip/cargo-member/releases>

## Usage

```console
$ cargo member --help
cargo-member 0.1.1
Ryo Yamashita <qryxip@gmail.com>
Cargo subcommand for managing workspace members.

USAGE:
    cargo member <SUBCOMMAND>

OPTIONS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    include    Include a package
    exclude    Exclude a workspace member
    focus      Include a package excluding the others
    new        Create a new workspace member with `cargo new`
    cp         Copy a workspace member
    rm         Remove a workspace member
    mv         Move a workspace member
    help       Prints this message or the help of the given subcommand(s)
```

### `cargo member include`

```console
$ tree "$PWD"
/home/ryo/src/local/workspace
├── a
│   ├── Cargo.toml
│   └── src
│       └── main.rs
└── Cargo.toml

2 directories, 3 files
$ cat ./Cargo.toml
[workspace]
$ cargo member include ./a
      Adding "a" to `workspace.members`
$ cat ./Cargo.toml
[workspace]
members = ["a"]
exclude = []
$ cargo metadata --format-version 1 | jq -r '.packages[] | .name'
a
```

### `cargo member exclude`

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
$ cargo member exclude ./a # or `-p a`
    Removing "a" from `workspace.members`
      Adding "a" to `workspace.exclude`
$ cat ./Cargo.toml
[workspace]
members = []
exclude = ["a"]
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
❯ tree "$PWD"
/home/ryo/src/local/workspace

0 directories, 0 files
$ echo '[workspace]' > ./Cargo.toml
$ cargo member new a
     Created binary (application) `/home/ryo/src/local/workspace/a` package
      Adding "a" to `workspace.members`
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
$ cargo metadata --format-version 1 | jq -r '.packages[] | .name'
a
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
├── Cargo.lock
└── Cargo.toml

2 directories, 4 files
$ cat ./Cargo.toml
[workspace]
members = ["a"]
exclude = []
$ cargo member rm ./a # or `-p a`
    Removing directory `/home/ryo/src/local/workspace/a`
    Removing "a" from `workspace.members`
$ tree "$PWD"
/home/ryo/src/local/workspace
├── Cargo.lock
└── Cargo.toml

0 directories, 2 files
$ cat ./Cargo.toml
[workspace]
members = []
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
