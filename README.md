# rcon-cli

A command-line RCON client for remote server administration.

This client was developed with Factorio in mind, but may work with other RCON-compatible games.

## Installation

### Homebrew (macOS/Linux)

```bash
brew tap connor-hanson/rcon-cli
brew install rcon-cli
```

### Cargo (crates.io)

```bash
cargo install rcon-cli
```

### From Source

```bash
cargo install --path .
```

## Features

- **Interactive Mode** - Connect once and run multiple commands in a persistent session
- **One-Shot Mode** - Execute single commands and exit
- **Config File Support** - Store server credentials in a JSON file for easy access
- **Lightweight** - Minimal dependencies and fast execution
- **Cross-Platform** - Works on macOS, Linux, and Windows

## Usage

### Interactive Mode

Connect to a server and run commands interactively:

```bach
> rcon-cli
Enter address: <your_server_address>:<your_server_port>
Enter password: <your_server_password>
[2026-02-14T01:13:27Z INFO  rcon_cli] Connected!
> <enter command>
```

```bash
rcon-cli --address <ip_addr> --port <port> --password <password> --show-responses
```

Example:
```bash
rcon-cli --address 127.0.0.1 --port 27015 --password mypassword
```

### One-Shot Mode

Execute a single command and exit:

```bash
rcon-cli --address <ip_addr> --port <port> --password <password> -c <command>
```

Example:
```bash
rcon-cli --address 127.0.0.1 --port 27015 --password mypassword -c "/players"
```

### [Optional] Configuration File

Store server credentials in a JSON config file to avoid typing them repeatedly.

#### Example Config File

Create a file (e.g., `~/.rcon-config.json`):

```json
{
  "configs": {
    "production": {
      "host": "192.168.1.100:27015",
      "password": "prod_password"
    },
    "staging": {
      "host": "192.168.1.101:27015",
      "password": "staging_password"
    },
    "local": {
      "host": "127.0.0.1:27015",
      "password": "dev_password"
    }
  }
}
```

#### Set Environment Variable

Point to your config file:

```bash
export RCON_CONFIG_PATH=~/.rcon-config.json
```

#### Using Configs
```bash
rcon-cli --config-name custom_config_profile
```

This loads the host and password from your config file, so you don't need to pass `--address` or `--password` flags.
