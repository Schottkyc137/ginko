# Ginko

A device-tree source parser, analyzer and language server.
The main goal of this project is to make working with device-trees easy.
For example, for the following device-tree:

```dts
/dts-v1/;

/ {
    pic@10000000 {
        phandle = <1>;
        interrupt-controller;
        reg = <0x10000000 0x100>;
    }
};
```

`dtc` produces the following output:

```
Error: test.dts:9.1-2 syntax error
FATAL ERROR: Unable to parse input tree
```

whereas `ginko` produces the following output:

```
 --> test.dts:8:6
  |
8 |     }
  |      ^ Expected ';'
```

At the moment, the command-line tool `ginko` only checks the device-tree source. 
It curently does not generate device-tree binary files nor can it output reformatted device-tree source files.

# Projects

## ginko

### Installation and Usage
To install ginko with the rust toolchain, simply call
```shell
cargo install ginko
```

Additionally, [pre-built binaries](https://github.com/Schottkyc137/ginko/releases/latest) exist for x86 Linux and x86 Windows.
Simply downloading them and adding the executables to a directory that is on the path should suffice to run the tool.
Pre-built binaries also exist for macOS running on Apple Silicon, however these are not signed and notarized with apple so installation is more tedious.
See [this](https://support.apple.com/guide/mac-help/apple-cant-check-app-for-malicious-software-mchleab3a043/mac) for more information.

Run
```shell
ginko <path/to/file.dts>
```
to run ginko on a device-tree source file and check the contents.

### Goals:

- A complete device-tree source parser.
- Error tolerant parsing for device-tree usage
- Providing readable and helpful feedback to the user

### Shortcomings

This project is in its infancy. Therefore, a couple of features aren't supported yet:

- C-style includes (i.e., `#include "some_header.h"`)
- Expressions (parenthesized expressions are ignored and do not throw an error)
- Binary Device Tree format
- Stable API

## ginko_ls

ginko_ls is meant to be a feature-complete language server for device-trees.
Language servers can be used in many editors, such as Visual Studio Code,
Emacs or Vim

### Features

- Outline
- Go to definition (nodes)
- hover

### Planned features

- Incremental analysis
- completion
- formatting

All contributions, whether in the form of Pull Requests or Issues are highly appreciated and welcome.
