# tuix

Pronunciation: /twɪks/

Terminal session multiplexer TUI. Run N concurrent terminal sessions in a tiled grid overview with one-action switching to full interactive mode.

Built for managing multiple instances of `claude`, `opencode`, or any shell-invoked tool side by side.

```
 ┌─ bash ──────────────┐┌─ claude ─────────────┐
 │ $ make build        ││ > Analyzing code...  │
 │ ==> Building...     ││                      │
 │                     ││                      │
 │  /home/user/project ││  /home/user/project  │
 └─────────────────────┘└─────────────────────-┘
 ┌─ opencode ──────────┐┌─ bash ──────────────-┐
 │ Ready.              ││ $ git status         │
 │                     ││ On branch main       │
 │                     ││                      │
 │  .../other-project  ││  .../other-project   │
 └─────────────────────┘└──────────────────────┘
```

Sessions sharing a working directory get matching colored borders for easy identification.

## Requirements

- [Podman](https://podman.io/) (no host Rust toolchain needed)
- Linux (PTY support required)

## Build

```sh
make build
```

This compiles the binary inside a CentOS Stream 10 container and exports it to `./tuix` via a podman named volume. No bind mounts are used.

## Usage

```sh
./tuix <session>... [--env KEY=VALUE]
```

Sessions are defined as positional arguments. Use `command@path` to specify a working directory, or just `command` to use the current directory.

```sh
# Three sessions in different directories
./tuix "claude@/home/user/project-a" "claude@/home/user/project-b" "bash"

# Two shells with an environment override
./tuix bash bash --env EDITOR=vim

# Mix of tools
./tuix "opencode@./frontend" "claude@./backend" "bash@./infra"

# Limit concurrent sessions and scrollback
./tuix bash --max-sessions 10 --scrollback 5000
```

### Options

| Flag             | Default | Description                              |
|------------------|---------|------------------------------------------|
| `--env KEY=VALUE`| —       | Environment variable override            |
| `--scrollback N` | 1000    | Scrollback lines per session (0 to disable) |
| `--max-sessions N` | 20   | Maximum concurrent sessions              |

## Controls

### Tile view (default)

| Key              | Action                      |
|------------------|-----------------------------|
| Click tile       | Focus that session          |
| Enter            | Focus selected tile         |
| Arrow keys / Tab | Navigate between tiles      |
| 0-9              | Focus session by index      |
| Ctrl+n           | Spawn new session           |
| Ctrl+w           | Remove dead session (y/n)   |
| Ctrl+b           | Toggle blur mode            |
| Ctrl+h           | Toggle help screen          |
| Ctrl+q           | Quit                        |

### Focus view (interactive)

| Key              | Action                      |
|------------------|-----------------------------|
| Ctrl+]           | Return to tile view         |
| Click [X]        | Return to tile view         |
| Scroll wheel     | Scroll through history      |
| Scrollbar drag   | Scroll through history      |
| Shift+PgUp/PgDn | Page through history        |
| Shift+Click/Drag | Select text (native)        |
| All other input  | Forwarded to session        |

## Other make targets

```sh
make test    # Build and run all tests in the container
make clean   # Remove binary, container images, and volume
make run     # Run the built binary (use ARGS= to pass arguments)
```

## License

- [License](LICENSE)
