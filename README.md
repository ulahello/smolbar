# smolbar

[![Crates.io](https://img.shields.io/crates/v/smolbar)](https://crates.io/crates/smolbar)
[![Crates.io](https://img.shields.io/crates/l/smolbar)](https://crates.io/crates/smolbar)

`smolbar` is a smol status command for [sway](https://github.com/swaywm/sway).

## milestones

- [X] refresh configurable blocks on signals/intervals
- [X] respond to stop and continue signals
- [ ] support click events

## installation

`smolbar` is on crates.io!

```console
$ cargo install --locked smolbar
```

# configuration

`smolbar` is configured through a TOML file.

if `--config` is not specified, `smolbar` looks for a file called `config.toml` in `$XDG_CONFIG_HOME/smolbar` or `$HOME/.config/smolbar`.

for an example of a configuration, see the [examples](./examples).

## header

the header first sent to sway can be configured in the `header` table.
it inherits all keys from the `Header` JSON object defined in `swaybar-protocol(7)`.

```toml
[header]
cont_signal = "SIGCONT" # default value
stop_signal = "SIGINT"
```

## blocks

there are three scopes which can be used to configure individual blocks.
each scope has a level of control over each block's `Body`: `immediate` has the highest precedence, then `local`, then `global`.

### global

the `global` scope is configured at the root level of the config file.

| key              | description                                                                                                                                                                                                           |
|------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| command\_dir     | sets the path to execute `local::command` in                                                                                                                                                                          |
| smolbar\_version | requires that current `smolbar` version satisfies the given version requirement (parsed according to [Cargo's flavor of Semantic Versioning](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)) |

`global` also inherits all the keys in the `Body` JSON object defined in `swaybar-protocol(7)`.

```toml
# global
full_text = "u only see this in a block if no other scopes define full_text"

[[block]]
# local
full_text = "never see global full_text"
```

### local

all `local` blocks are tables in the table array `block`.

| key      | description                                         |
|----------|-----------------------------------------------------|
| command  | command to execute for `immediate` configuration    |
| prefix   | string prefixing `full_text`                        |
| postfix  | string appended to `full_text`                      |
| interval | interval, in seconds, at which to refresh the block |
| signal   | OS signal name to refresh the block when received   |

`local` inherits all other keys from the `Body` JSON object defined in `swaybar-protocol(7)`.

```toml
[[block]]
# this block displays the date, updating every second
command = "date" # assuming date coreutil is in $PATH
prefix = "Date: "
interval = 1
```

### immediate

each line of `local::command`'s standard output is parsed in order as a field of the `Body` JSON object defined in `swaybar-protocol(7)`.

for example, suppose the following script was a block's command:

```sh
# interpreted as `full_text`
echo "amazing status information"

# interpreted as `short_text`
echo "short info"

# interpreted as `color`
echo "#ff0000"
```

## hot swapping

`smolbar` responds to `cont_signal` (see `swaybar-protocol(7)`) by reloading its configuration.

this means that by default, sending `smolbar`'s process `SIGCONT` will cause it to hot swap its config.

```toml
[header]
# cont_signal is SIGCONT by default
```

```console
$ pkill -SIGCONT smolbar
# causes smolbar to reload config
```

`cont_signal` is also configurable.

```toml
[header]
cont_signal = "SIGUSR1"
```

```console
$ pkill -SIGUSR1 smolbar
# causes smolbar to reload config
```

### note

the header, fundamentally, can't be reconfigured during runtime.

this is because in `swaybar-protocol(7)`, it's only sent once, at the beginning of the status command's process.

## security considerations

**by nature, `smolbar` executes arbitrary code** as defined in its configuration file.

if an attacker can write to the configuration file, or to *any* of the files defined as commands, that attacker is able to execute arbitrary code (either immediately or when the config is reloaded).

it is **your responsibility** to prevent this.
it's a good idea to ensure that no other users are granted write permissions for the config file or its commands.
however, measures you take will **depend on your situation**.
