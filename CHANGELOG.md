# changelog

## [unreleased]

## [0.3.1] - 2022-06-28
* fixed incorrect command_dir
  * previously the configuration path wasn't canonicalized
* fixed trace log from dependency showing on program end
* fixed fatal errors not having timestamps

## [0.3.0] - 2022-06-08
* added support for floating point intervals

## [0.2.0] - 2022-06-08
* added crate documentation
* added timestamps to logs
* fixed slow shutdown with slow command
* fixed incorrect documentation in README

## [0.1.1] - 2022-06-04
* fixed panic when receiving continue signal and stop signal simultaneously
* improved portability of exit code

## [0.1.0] - 2022-06-02
* feat: refresh configurable blocks on signals/intervals
* feat: add `--config` & `--license` cli flags
