# Overview

`rcon-rs` is a Rust implementation of the RCON protocol. 
This client was developed with Factorio in mind. 

If you notice that the client works with games besides Factorio, please raise a PR to document this. 

## Supported Games
|---|
|Factorio|

## Use

The package has two modes: 
1) Library
2) CLI

To use the CLI interface, simply run `rcon-rs --address <ip_addr> --port <port> --password <pw>`
You may also run the commands as a one-shot: `rcon-rs --address <ip_addr> --port <port> --password <pw> -c <command_str>`
