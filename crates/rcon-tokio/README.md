# Overview

RCON protocol implementation. 

## Installation

```
cargo add rcon-tokio
```

or add the following line to your Cargo.toml

```
rcon-tokio = "0.1.0"
```

## Usage

```rust
use rcon_tokio::{RconClient, errors::RconError};

let mut client = RconClient::connect("127.0.0.1:27015");
client.auth("myPassword").await?; // throws RconError if failed to auth
client.exec("my command to server").await?; 
```

## Contributions

This RCON client was developed with Factorio / MacOS in mind. 

If you notice any issues, please raise an issue here: https://github.com/connor-hanson/rcon-rs
