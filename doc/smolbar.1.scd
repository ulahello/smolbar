smolbar(1)

# NAME

smolbar - smol status command for *sway*(1)

# SYNOPSIS

*smolbar* [_OPTIONS_]

# DESCRIPTION

; TODO: consider adding section on mental model

*smolbar* is a status command for the sway Wayland compositor.

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

# AUTHORS

Maintained by Ula Shipman <ula.hello@mailbox.org>.

Source code and issue tracker can be found at
<https://github.com/ulahello/smolbar>.

# SEE ALSO

*smolbar*(5), *swaybar-protocol*(7), *sway*(5), *sway-bar*(5)
