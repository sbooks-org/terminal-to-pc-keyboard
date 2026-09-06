# terminal-to-pc-keyboard

Transport-neutral host-keyboard mapper for IBM PC/XT and PC/AT Set 1 keyboard-controller input. It accepts normalized host press, repeat, and release events and emits the exact guest scan bytes to inject into an emulator.

The library contains no terminal input implementation. A host application owns terminal setup and parsing; ncurses may remain responsible for display only.

## Add as an 86Box submodule

```sh
git submodule add git@github.com:sbooks-org/terminal-to-pc-keyboard.git third_party/terminal-to-pc-keyboard
git submodule update --init --recursive
```

Build the static library from the 86Box build or dependency step:

```sh
cargo build --manifest-path third_party/terminal-to-pc-keyboard/Cargo.toml --release
```

This produces:

```text
third_party/terminal-to-pc-keyboard/target/release/libpc_xt_keyboard.a
third_party/terminal-to-pc-keyboard/include/pc_xt_keyboard.h
```

Link `libpc_xt_keyboard.a` and include `pc_xt_keyboard.h`. Cargo/rustc also determines any target-specific system libraries needed by the static artifact; use the same Rust target triple as the 86Box build target.

## C ABI

The versioned ABI is declared in `include/pc_xt_keyboard.h`. It exposes opaque mapper state and raw scan-byte output:

```c
void *keyboard = pc_xt_keyboard_v1_create(PC_XT_KEYBOARD_V1_AT_SET1);
uint8_t bytes[PC_XT_KEYBOARD_V1_EVENT_MAX_BYTES];

pc_xt_keyboard_v1_input_event event = {
    .key = {
        .kind = PC_XT_KEYBOARD_V1_KEY_CHAR,
        .character = 'a',
    },
    .kind = PC_XT_KEYBOARD_V1_PRESS,
};

size_t count = pc_xt_keyboard_v1_handle(
    keyboard, &event, bytes, sizeof(bytes)
);
if (count != PC_XT_KEYBOARD_V1_ERROR) {
    /* Inject bytes[0..count] into the emulated keyboard controller. */
}

pc_xt_keyboard_v1_destroy(keyboard);
```

`pc_xt_keyboard_v1_handle` requires an output buffer of at least `PC_XT_KEYBOARD_V1_EVENT_MAX_BYTES` bytes. It returns the emitted byte count, or `PC_XT_KEYBOARD_V1_ERROR` for invalid input, null pointers, or an undersized buffer. Error returns leave mapper state unchanged.

The API uses only C fixed-width integers. It does not expose Rust allocations, Rust enum layout, or Rust-owned strings. Every exported symbol is prefixed `pc_xt_keyboard_v1_`; incompatible future APIs will use a new versioned prefix.

## Input boundary

The C caller converts its input source into `pc_xt_keyboard_v1_input_event`:

- `key`: a semantic host key. Characters use a Unicode scalar in `character`; function and modifier keys use `value`.
- `modifiers`: `SHIFT`, `CONTROL`, `ALT`, and `SUPER` bits.
- `kind`: `PRESS`, `REPEAT`, or `RELEASE`.

Send every actual press and release to the same mapper instance. It maintains held-key state and emits break sequences only when the final source holding a guest key is released.

For terminal applications, use ncurses for output only. Read stdin in raw mode, negotiate and parse a keyboard protocol such as kitty when available, then convert parsed events into this ABI. Legacy terminal fallback can map ordinary characters and traditional escape sequences, but it cannot recover key releases or bare modifier transitions that the terminal never sent. A fallback may synthesize a matching release after a timeout, with the usual limitations for games and held modifiers.

## Keyboard models

- `PC_XT_KEYBOARD_V1_XT_SET1` maps the 83-key PC/XT model.
- `PC_XT_KEYBOARD_V1_AT_SET1` emits AT-specific Set 1 sequences where required.

For example, AT Print Screen emits `E0 2A E0 37` on press and `E0 B7 E0 AA` on release. AT Pause emits `E1 1D 45 E1 9D C5` and has no break sequence. The MacBook command-layer mapping can emit AT SysRq as `54` when the caller supplies the corresponding `SUPER | CONTROL | SHIFT | ALT` event.

## Development checks

```sh
cargo test
cargo test --all-features
cargo build --release --all-features
```

`tests/c_api_smoke.c` is a C consumer test covering XT/AT byte output through the public header and static library.

## License

[MIT-0](LICENSE). Copyright (C) 2026 Simplebooks Foundation and Copyright (C) 2026 Josh Rodd.
