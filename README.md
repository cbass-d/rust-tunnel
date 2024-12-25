Very early WIP of server used for SSH tunneling for file transfers and communication.
Currently mostly made up of code from 'russh' example.

### Config File
'confy' crate is being used for application configuration:
* Default listening port is 2222.
* Default server key is auto-generated at every startup.

### Running
Using cargo:
```
$ cargo run   # Uses port specified in config file
$ cargo run -- --port ${port-number}    # Custom port
```


