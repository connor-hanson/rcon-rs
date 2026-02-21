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

## QuickStart

```rust
use rcon_tokio::{RconClient, RconClientConfig, errors::RconError};

let rcon_client_config = RconClientConfig::new(
    "my_server_host",
    "my_server_port",
    "my_server_password",
)
// Some servers split responses. This is how long to wait between responses for the next
// Values above 200 ms not recommended.
.idle_timeout(Duration::from_millis(123)) 
// How long to wait before timing out a request.
// This is distinct from idle_timeout, in that it causes an error.
.io_timeout(Duration::from_millis(123))
// Maximum times to attempt server reconnect on failed command
.max_reconnect_attempts(3)
// Reconnect to server on failed command?
.auto_reconnect(true);

let mut client = RconClient::connect(rcon_client_config).await?;
client.execute("myCommand").await?;
```

## Contributions

This RCON client was developed with Factorio / MacOS in mind. 

If you notice any issues, please raise an issue here: https://github.com/connor-hanson/rcon-rs
