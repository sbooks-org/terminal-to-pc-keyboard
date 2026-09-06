use std::{ffi::c_void, ptr};

use crate::{InputEvent, InputKey, InputKind, KeyboardModel, ModifierKey, Modifiers, PcEvent, PcKeyboard};

pub const EVENT_MAX_BYTES: usize = 32;
pub const ERROR: usize = usize::MAX;
#[derive(Clone, Copy)]

#[repr(C)]
pub struct CInputKey {
    kind: u32,
    value: u32,
    character: u32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct CInputEvent {
    key: CInputKey,
    modifiers: u8,
    kind: u8,
}

fn decode_key(key: CInputKey) -> Option<InputKey> {
    match key.kind {
        0 => char::from_u32(key.character).map(InputKey::Char),
        1 => Some(InputKey::Backspace),
        2 => Some(InputKey::Delete),
        3 => Some(InputKey::Insert),
        4 => Some(InputKey::Enter),
        5 => Some(InputKey::Left),
        6 => Some(InputKey::Right),
        7 => Some(InputKey::Up),
        8 => Some(InputKey::Down),
        9 => Some(InputKey::Home),
        10 => Some(InputKey::End),
        11 => Some(InputKey::PageUp),
        12 => Some(InputKey::PageDown),
        13 => Some(InputKey::PrintScreen),
        14 => Some(InputKey::Pause),
        15 => Some(InputKey::ScrollLock),
        16 => Some(InputKey::NumLock),
        17 => Some(InputKey::KeypadBegin),
        18 => Some(InputKey::Escape),
        19 => Some(InputKey::Null),
        20 => u8::try_from(key.value).ok().map(InputKey::Function),
        21 => match key.value {
            0 => Some(InputKey::Modifier(ModifierKey::LeftShift)),
            1 => Some(InputKey::Modifier(ModifierKey::RightShift)),
            2 => Some(InputKey::Modifier(ModifierKey::LeftControl)),
            3 => Some(InputKey::Modifier(ModifierKey::RightControl)),
            4 => Some(InputKey::Modifier(ModifierKey::LeftAlt)),
            5 => Some(InputKey::Modifier(ModifierKey::RightAlt)),
            6 => Some(InputKey::Modifier(ModifierKey::LeftSuper)),
            7 => Some(InputKey::Modifier(ModifierKey::RightSuper)),
            8 => Some(InputKey::Modifier(ModifierKey::Other)),
            _ => None,
        },
        _ => None,
    }
}

fn decode_event(input: CInputEvent) -> Option<InputEvent> {
    let key = decode_key(input.key)?;
    if input.modifiers & !0x0F != 0 {
        return None;
    }
    let kind = match input.kind {
        0 => InputKind::Press,
        1 => InputKind::Repeat,
        2 => InputKind::Release,
        _ => return None,
    };
    let mut modifiers = Modifiers::NONE;
    for (bit, modifier) in [
        (1, Modifiers::SHIFT),
        (2, Modifiers::CONTROL),
        (4, Modifiers::ALT),
        (8, Modifiers::SUPER),
    ] {
        if input.modifiers & bit != 0 {
            modifiers |= modifier;
        }
    }
    Some(InputEvent::new(key, modifiers, kind))
}

fn flatten(events: Vec<PcEvent>) -> Vec<u8> {
    let mut bytes = Vec::new();
    for event in events {
        match event {
            PcEvent::Make(key) => bytes.extend_from_slice(key.make.bytes()),
            PcEvent::Break(key) => bytes.extend_from_slice(key.break_sequence.bytes()),
        }
    }
    bytes
}
/// Creates a version-1 C ABI mapper. Returns null for an unknown model.
#[unsafe(no_mangle)]
pub extern "C" fn pc_xt_keyboard_v1_create(model: u32) -> *mut c_void {
    let model = match model {
        0 => KeyboardModel::XtSet1,
        1 => KeyboardModel::AtSet1,
        _ => return ptr::null_mut(),
    };
    Box::into_raw(Box::new(PcKeyboard::new(model))).cast()
}

/// Destroys a mapper returned by [`pc_xt_keyboard_v1_create`]. Null is accepted.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pc_xt_keyboard_v1_destroy(keyboard: *mut c_void) {
    if !keyboard.is_null() {
        unsafe { drop(Box::from_raw(keyboard.cast::<PcKeyboard>())) };
    }
}

/// Processes one event and copies its emitted scan bytes into `output`.
///
/// `output_capacity` must be at least [`EVENT_MAX_BYTES`]. On invalid input, a null pointer,
/// or an undersized output buffer, returns [`ERROR`] and leaves mapper state unchanged.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pc_xt_keyboard_v1_handle(
    keyboard: *mut c_void,
    input: *const CInputEvent,
    output: *mut u8,
    output_capacity: usize,
) -> usize {
    if keyboard.is_null() || input.is_null() || output.is_null() || output_capacity < EVENT_MAX_BYTES {
        return ERROR;
    }
    let Some(input) = (unsafe { input.as_ref() }).copied().and_then(decode_event) else {
        return ERROR;
    };
    let keyboard = unsafe { &mut *keyboard.cast::<PcKeyboard>() };
    let bytes = flatten(keyboard.handle(&input));
    debug_assert!(bytes.len() <= EVENT_MAX_BYTES);
    unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len()) };
    bytes.len()
}
