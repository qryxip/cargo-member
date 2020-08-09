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
cargo-member 0.1.2
Ryo Yamashita <qryxip@gmail.com>
Cargo subcommand for managing workspace members.

USAGE:
    cargo member <SUBCOMMAND>

OPTIONS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    include       Add a package to `workspace.members`
    exclude       Move a package from `package.members` to `workspace.exclude`
    deactivate    Remove a package from both of `package.{members, exclude}`
    focus         `include` a package and `deactivate`/`exclude` the others
    new           Create a new workspace member with `cargo new`
    cp            Copy a workspace member
    rm            Remove a workspace member
    mv            Move a workspace member
    help          Prints this message or the help of the given subcommand(s)
```

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
$ cargo metadata --format-version 1 | jq -r '.packages | map(.id) | sort[]'
a 0.1.0 (path+file:///home/ryo/src/local/workspace/a)
$ cargo member include ./b
      Adding "b" to `workspace.members`
    Updating /home/ryo/src/local/workspace/Cargo.lock
$ cat ./Cargo.toml
[workspace]
members = ["a", "b"]
exclude = []
$ cargo metadata --format-version 1 | jq -r '.packages | map(.id) | sort[]'
a 0.1.0 (path+file:///home/ryo/src/local/workspace/a)
b 0.1.0 (path+file:///home/ryo/src/local/workspace/b)
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
$ cargo metadata --format-version 1 | jq -r '.packages | map(.id) | sort[]'
a 0.1.0 (path+file:///home/ryo/src/local/workspace/a)
b 0.1.0 (path+file:///home/ryo/src/local/workspace/b)
$ cargo member exclude ./b # or `-p b`
    Removing "b" from `workspace.members`
      Adding "b" to `workspace.exclude`
    Updating /home/ryo/src/local/workspace/Cargo.lock
$ cat ./Cargo.toml
[workspace]
members = ["a"]
exclude = ["b"]
$ cargo metadata --format-version 1 | jq -r '.packages | map(.id) | sort[]'
a 0.1.0 (path+file:///home/ryo/src/local/workspace/a)
```

### `cargo member deactivate`

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
members = ["a", "b"]
exclude = ["c"]
$ cargo metadata --format-version 1 | jq -r '.packages | map(.id) | sort[]'
a 0.1.0 (path+file:///home/ryo/src/local/workspace/a)
b 0.1.0 (path+file:///home/ryo/src/local/workspace/b)
$ cargo member deactivate ./b ./c
    Removing "b" from `workspace.members`
    Removing "c" from `workspace.exclude`
    Updating /home/ryo/src/local/workspace/Cargo.lock
$ cat ./Cargo.toml
[workspace]
members = ["a"]
exclude = []
$ cargo metadata --format-version 1 | jq -r '.packages | map(.id) | sort[]'
a 0.1.0 (path+file:///home/ryo/src/local/workspace/a)
$ cargo metadata --format-version 1 --manifest-path ./b/Cargo.toml
error: current package believes it's in a workspace when it's not:
current:   /home/ryo/src/local/workspace/b/Cargo.toml
workspace: /home/ryo/src/local/workspace/Cargo.toml

this may be fixable by adding `b` to the `workspace.members` array of the manifest located at: /home/ryo/src/local/workspace/Cargo.toml
Alternatively, to keep it out of the workspace, add the package to the `workspace.exclude` array, or add an empty `[workspace]` table to the package's manifest.
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
$ cargo metadata --format-version 1 | jq -r '.packages | map(.id) | sort[]'
a 0.1.0 (path+file:///home/ryo/src/local/workspace/a)
b 0.1.0 (path+file:///home/ryo/src/local/workspace/b)
c 0.1.0 (path+file:///home/ryo/src/local/workspace/c)
$ cargo member focus ./a # or `-p a`
    Removing "b" from `workspace.members`
    Removing "c" from `workspace.members`
    Updating /home/ryo/src/local/workspace/Cargo.lock
$ cat ./Cargo.toml
[workspace]
members = ["a"]
exclude = []
$ cargo metadata --format-version 1 | jq -r '.packages | map(.id) | sort[]'
a 0.1.0 (path+file:///home/ryo/src/local/workspace/a)
```

### `cargo member new`

```console
$ tree "$PWD"
/home/ryo/src/local/workspace

0 directories, 0 files
$ echo '[workspace]' > ./Cargo.toml
$ cargo member new a
      Adding "a" to `workspace.members`
     Created binary (application) `/home/ryo/src/local/workspace/a` package
    Updating /home/ryo/src/local/workspace/Cargo.lock
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
$ cargo metadata --format-version 1 | jq -r '.packages | map(.id) | sort[]'
a 0.1.0 (path+file:///home/ryo/src/local/workspace/a)
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
$ cargo metadata --format-version 1 | jq -r '.packages | map(.id) | sort[]'
a 0.1.0 (path+file:///home/ryo/src/local/workspace/a)
$ cargo member cp a ./b
     Copying `/home/ryo/src/local/workspace/a` to `/home/ryo/src/local/workspace/b`
       Found workspace at /home/ryo/src/local/workspace
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
$ cargo metadata --format-version 1 | jq -r '.packages | map(.id) | sort[]'
a 0.1.0 (path+file:///home/ryo/src/local/workspace/a)
b 0.1.0 (path+file:///home/ryo/src/local/workspace/b)
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
$ cargo metadata --format-version 1 | jq -r '.packages | map(.id) | sort[]'
a 0.1.0 (path+file:///home/ryo/src/local/workspace/a)
b 0.1.0 (path+file:///home/ryo/src/local/workspace/b)
$ cargo member rm ./b # or `-p b`
    Removing directory `/home/ryo/src/local/workspace/b`
    Removing "b" from `workspace.members`
    Updating /home/ryo/src/local/workspace/Cargo.lock
$ cat ./Cargo.toml
[workspace]
members = ["a"]
exclude = []
$ cargo metadata --format-version 1 | jq -r '.packages | map(.id) | sort[]'
a 0.1.0 (path+file:///home/ryo/src/local/workspace/a)
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
$ cargo metadata --format-version 1 | jq -r '.packages | map(.id) | sort[]'
a 0.1.0 (path+file:///home/ryo/src/local/workspace/a)
$ cargo member mv a ./b
     Copying `/home/ryo/src/local/workspace/a` to `/home/ryo/src/local/workspace/b`
       Found workspace at /home/ryo/src/local/workspace
      Adding "b" to `workspace.members`
    Removing directory `/home/ryo/src/local/workspace/a`
    Removing "a" from `workspace.members`
    Updating /home/ryo/src/local/workspace/Cargo.lock
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
$ cargo metadata --format-version 1 | jq -r '.packages | map(.id) | sort[]'
b 0.1.0 (path+file:///home/ryo/src/local/workspace/b)
```

## License

Licensed under <code>[MIT](https://opensource.org/licenses/MIT) OR [Apache-2.0](http://www.apache.org/licenses/LICENSE-2.0)</code>.
