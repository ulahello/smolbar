smolbar(1)

# NAME

smolbar - smol status command for *sway*(1)

# SYNOPSIS

*smolbar* [_OPTIONS_]

# DESCRIPTION

*smolbar* is a status command for the sway Wayland compositor. When
run, *smolbar* will write JSON _Block_ objects to standard output, as
per *swaybar-protocol*(7).

To use it in your sway configuration, invoke _bar_ _status_command_ with
_smolbar_.

```
bar {
	status_command smolbar
	# the rest of your bar config
}
```

# OPTIONS

*-c*, *--config*=_PATH_
	Path to configuration file, see *smolbar*(5) for details.

	Default: _config.toml_ in _$XDG_CONFIG_HOME/smolbar_ or otherwise
	_$HOME/.config/smolbar_.

*-t*, *--terse*
	Decrease log verbosity.

*-l*, *--license*
	Print license information.

*-V*, *--version*
	Print smolbar version.

*--help*
	Display usage information.

# MENTAL MODEL

This is not an explanation of the configuration file, see *smolbar*(5) for that.

*smolbar* fulfills the role described by *swaybar-protocol*(7). The user
controls *smolbar*'s behavior through its configuration file. Note that its
configurable behavior is a superset of the behavior outlined in the protocol.

## BLOCK

A block is a unit of refreshable content. The structure of this content is
defined by the _Body_ JSON object from the protocol, with additional information
to make them useful. To be dynamic, blocks require both a "what" (a source of
content) and a "when" (when to refresh the content).

The "what" is implemented by giving blocks a command to execute. The entire
_Body_ JSON object is filled in with the output of this command.

The "when" is currently implemented in two ways: periodic intervals and
operating system signals. This means that a block's content gets refreshed on a
timer and whenever *smolbar* receives a specific signal.

## BAR

The bar is the owner of blocks. The core behavior of *smolbar* is to send the
bar's blocks whenever a block has new content.

The bar is also responsible for responding to _cont_signal_ and _stop_signal_,
which it sends in the _Header_ JSON object (also from the protocol). If it
receives _stop_signal_, *smolbar* will gracefully shut down, as per spec. Upon
receiving _cont_signal_, *smolbar* will reload its configuration. Note that
*smolbar* has given new meaning to _cont_signal_, since the meaning described by
the protocol isn't particularly applicable.

# AUTHORS

Maintained by Ula Shipman <ula.hello@mailbox.org>.

Source code and issue tracker can be found at
<https://github.com/ulahello/smolbar>.

# SEE ALSO

*smolbar*(5), *swaybar-protocol*(7), *sway*(5), *sway-bar*(5)
