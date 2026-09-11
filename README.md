# terminal-to-pc-keyboard
v0.1.0. 
Last updated 11 September 2026

## License

[MIT-0](LICENSE). 
Copyright (C) 2026 Simplebooks Foundation

Transport-neutral host-keyboard mapper for IBM PC/XT and PC/AT keyboards. It accepts normalized host press, repeat, and release events and emits physical guest key transitions. Set 1 scan-byte output remains available for consumers that implement the keyboard wire protocol.

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

The versioned ABI is declared in `include/pc_xt_keyboard.h`. It exposes opaque mapper state and caller-owned physical key events:

```c
void *keyboard = pc_xt_keyboard_v1_create(PC_XT_KEYBOARD_V1_AT_SET1);
pc_xt_keyboard_v1_key_event keys[PC_XT_KEYBOARD_V1_EVENT_MAX_KEYS];

pc_xt_keyboard_v1_input_event event = {
    .key = {
        .kind = PC_XT_KEYBOARD_V1_KEY_CHAR,
        .character = 'a',
    },
    .kind = PC_XT_KEYBOARD_V1_PRESS,
};

size_t count = pc_xt_keyboard_v1_handle_events(
    keyboard, &event, keys, sizeof(keys) / sizeof(keys[0])
);
if (count != PC_XT_KEYBOARD_V1_ERROR) {
    /* Deliver keys[0..count): key is the physical identity, down is 1 or 0. */
}

pc_xt_keyboard_v1_destroy(keyboard);
```

`pc_xt_keyboard_v1_handle_events` measures capacity in event slots, not bytes, and requires at least `PC_XT_KEYBOARD_V1_EVENT_MAX_KEYS` (64) slots. This conservatively bounds every current mapping: a bulk Command release emits at most 37 transitions, and other inputs emit at most eight. It returns the emitted event count, or `PC_XT_KEYBOARD_V1_ERROR` for invalid input, null pointers, or insufficient capacity. Error returns leave mapper state and output unchanged, so the input can be retried. Bulk releases are also checked against the actual caller capacity before mutation, guarding against overflow if future mappings expand.

The caller owns both input and output memory; the mapper retains neither pointer. Pass properly aligned, readable input and writable output slots that do not overlap each other or mapper state. Only the returned slots are written. Serialize access to each mapper and destroy it with `pc_xt_keyboard_v1_destroy`; no output allocation needs freeing.

For wire-protocol consumers, `pc_xt_keyboard_v1_handle` still emits the original Set 1 bytes into a `uint8_t` buffer, with capacity measured in bytes and a minimum of `PC_XT_KEYBOARD_V1_EVENT_MAX_BYTES`. Both APIs consume the same held-key state: call one API per input transition, not both for the same input. Physical events are emitted directly from mapping decisions, without decoding the byte representation.

The API uses only C fixed-width integers. It does not expose Rust allocations, Rust enum layout, or Rust-owned strings. Every exported symbol is prefixed `pc_xt_keyboard_v1_`; incompatible future APIs will use a new versioned prefix.

## Rust API

`PcKeyboard::handle` returns owned `PcEvent::Make(PcKey)` and `PcEvent::Break(PcKey)` values:

```rust
use pc_xt_keyboard::{InputEvent, InputKey, InputKind, Modifiers, PcEvent, PcKeyboard};

let mut keyboard = PcKeyboard::default();
let input = InputEvent::new(InputKey::Char('a'), Modifiers::NONE, InputKind::Press);
for event in keyboard.handle(&input) {
    let (physical, down) = match event {
        PcEvent::Make(key) => (key.physical, true),
        PcEvent::Break(key) => (key.physical, false),
    };
    assert_eq!((physical, down), (0x1e, true));
}
```

`PcKey::make` and `PcKey::break_sequence` retain the complete Set 1 sequences for wire-protocol consumers. Physical consumers use `PcKey::physical` instead; they must not treat it as a byte to inject into a controller.

## Input boundary

The C caller converts its input source into `pc_xt_keyboard_v1_input_event`:

- `key`: a semantic host key. Characters use a Unicode scalar in `character`; function and modifier keys use `value`.
- `modifiers`: `SHIFT`, `CONTROL`, `ALT`, and `SUPER` bits.
- `kind`: `PRESS`, `REPEAT`, or `RELEASE`.

Send every actual press and release to the same mapper instance. It maintains held-key state and emits a release only when the final source holding a guest key is released. Repeats are ignored. Releasing Command (`SUPER`) also releases dependent command-layer mappings.

For terminal applications, use ncurses for output only. Read stdin in raw mode, negotiate and parse a keyboard protocol such as kitty when available, then convert parsed events into this ABI. Legacy terminal fallback can map ordinary characters and traditional escape sequences, but it cannot recover key releases or bare modifier transitions that the terminal never sent. A fallback may synthesize a matching release after a timeout, with the usual limitations for games and held modifiers.

## Keyboard models

- `PC_XT_KEYBOARD_V1_XT_SET1` maps the 83-key PC/XT model.
- `PC_XT_KEYBOARD_V1_AT_SET1` selects enhanced AT Print Screen and Pause identities and their Set 1 sequences.

Ordinary physical identities are PC make-position numbers `0x01..0x7f`, including keypad navigation and SysRq (`0x54`). AT Print Screen is `0x137`, distinct from keypad multiply (`0x37`), and AT Pause is `0x145`. XT Print Screen is `0x37`; XT Pause maps to Ctrl (`0x1d`) plus Num Lock (`0x45`). These identities are not encoded scan sequences; a controller adapter owns translation to its actual keyboard protocol.

AT navigation outside the Command layer uses distinct enhanced keys: Home `0x147`, Up `0x148`, Page Up `0x149`, Left `0x14b`, Right `0x14d`, End `0x14f`, Down `0x150`, Page Down `0x151`, Insert `0x152`, and Delete `0x153`. Their wire sequences are `E0 position` on press and `E0 (position | 80)` on release, so navigation remains navigation with Num Lock enabled. XT navigation, Command-layer keypad mappings, and explicit keypad function-key mappings retain their unextended identities and bytes.

For wire-protocol consumers, AT Print Screen emits `E0 2A E0 37` on press and `E0 B7 E0 AA` on release. AT Pause emits `E1 1D 45 E1 9D C5` and has no wire break sequence, but the physical API still emits its matching release. The MacBook command-layer mapping can emit AT SysRq as `54` when the caller supplies the corresponding `SUPER | CONTROL | SHIFT | ALT` event.

## Development checks

```sh
cargo test
cargo test --all-features
cargo build --release --all-features
```

`tests/public_api.rs` covers Rust physical identity and release policy alongside the original byte sequences. `tests/c_api_smoke.c` exercises the public C header and library, including XT/AT identities, shared modifiers, error-state preservation, bulk-release capacity retries, and original byte output. After `cargo build`, a macOS C consumer check is:

```sh
cc -std=c11 -Wall -Wextra -Werror -Iinclude tests/c_api_smoke.c \
  -Ltarget/debug -lpc_xt_keyboard -Wl,-rpath,"$PWD/target/debug" \
  -o target/c_api_smoke
target/c_api_smoke
```

