Very early WIP of server used for SSH tunneling for file transfers and communication.
Currently mostly made up of code from 'russh' example.

### Config File
'confy' crate is being used for application configuration
Config file is stored under /home/${user}/.config/rust-tunnel/rustunnel-conf.toml
Can currently configure:
* Listening port
    * Default is 2222
* Inactivity timeout
    * Default is 3600 seconds
* Authentication rejection time
    * Default is 3 seconds 
* Server keys. Specified with file paths. Currently only OpenSSH PEM files
    * By default a new key is generated at every startup

### Running
Using cargo:
```
$ cargo run   # Uses port specified in config file
$ cargo run -- --port ${port-number}    # Custom port
```

### SFTP
SFTP can be used using an SFTP client, an example is:
```
$ sftp -P {port} {server address}
```
