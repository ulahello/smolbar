smolbar(5)

# NAME

smolbar - configuration file

# DESCRIPTION

*smolbar* is configured in a TOML file.

# HEADER

_Header_ is a JSON object defined by *swaybar-protocol*(7).

The _Header_ first sent to *sway*(1) can be configured in the _header_
TOML table. It inherits all keys verbatim from _Header_.

For example:

```
[header]
cont_signal = "SIGCONT" # default value, as per swaybar-protocol(7)
stop_signal = "SIGINT"
```

# BLOCKS

There are three scopes at which the content and behavior of blocks can
be defined.

- "Global" scope has the lowest precedence, but applies to all blocks.
- "Local" scope is defined per block.
- "Immediate" scope has the highest precedence, and is defined per
  block, but by the block's command.

This means that the global and local scopes can be used to give _Body_
properties default values, while immediate scopes are useful for
properties that change.

## GLOBAL SCOPE

The global scope is configured at the root level of the configuration
file.

[[ Key
:- Type
:< Description
|[ _command_dir_
:  string
:  sets the directory in which to execute the command (defined in
   local scope)
|  _smolbar_version_
:  string
:  requires that current *smolbar* version satisfies the given version
   requirement (parsed according to Cargo's flavor of Semantic
   Versioning, see https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)

The global scope also inherits all the properties from the _Body_ JSON
object defined by *swaybar-protocol*(7).

For example:

```
# global
full_text = "only visible if no other scopes define full_text"

[[block]]
# local
full_text = "never see global full_text"
```

## LOCAL SCOPE

All local scopes are tables in the table array _block_.

[[ Key
:- Type
:< Description
|[ _command_
:  string
:  path of command to execute in full for new content
|  _prefix_
:  string
:  prefixes _full_text_
|  _postfix_
:  string
:  appended to _full_text_
|  _interval_
:  number
:  interval, in seconds, at which to periodically refresh the block
|  _signal_
:  string
:  operating system signal to refresh the block when received (see
   section SUPPORTED SIGNALS)

The local scope inherits all other keys from _Body_.

For example:

```
[[block]]
# this block displays the date every second
command = "date" # assuming date coreutil is in $PATH
prefix = "Today is "
interval = 1
```

## IMMEDIATE SCOPE

Each line of the executed _command_'s (defined in local scope)
standard output is parsed in order as a _Body_ property. The order is
the same as they appear in *swaybar-protocol*(7).

For example, suppose the following script was a block's command:

```
# interpreted as `full_text`
echo 'amazing status information'

# interpreted as `short_text`
echo 'short info'

# interpreted as `color`
echo '#ff0000'
```

# HOT SWAPPING

*smolbar* responds to _cont_signal_ by reloading its configuration.

This means that by default, sending *smolbar*'s process _SIGCONT_ will
cause it to hot swap its configuration.

```
[header]
# cont_signal is SIGCONT by default, as per swaybar-protocol(7)
```

```
$ pkill -SIGCONT smolbar
# causes smolbar to reload config
```

_cont_signal_ is also configurable.

```
[header]
cont_signal = "SIGUSR1"
```

```
$ pkill -SIGUSR1 smolbar
# causes smolbar to reload config
```

Note that the header cannot be reconfigured during runtime. This is
because in *swaybar-protocol*(7), it's only sent once, at the
beginning of the status command's process.

# SUPPORTED SIGNALS

The following operating system signals are currently supported:

- _SIGALRM_
- _SIGCHLD_
- _SIGCONT_
- _SIGHUP_
- _SIGINT_
- _SIGIO_
- _SIGPIPE_
- _SIGQUIT_
- _SIGSTOP_
- _SIGTERM_
- _SIGUSR1_
- _SIGUSR2_
- _SIGWINCH_

# SEE ALSO

*smolbar*(1), *swaybar-protocol*(7)
