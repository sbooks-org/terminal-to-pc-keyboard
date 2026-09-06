use std::{
    collections::{HashMap, HashSet},
    io::{self, Stdout, Write},
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{
        self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
        ModifierKeyCode, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute, queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use pc_xt_keyboard::{self as xt, KeyboardModel, PcEvent, PcKeyboard};

const SIMULATED_RELEASE_DELAY: Duration = Duration::from_millis(120);

struct ScreenGuard;

impl ScreenGuard {
    fn enter(stdout: &mut Stdout) -> io::Result<Self> {
        execute!(stdout, EnterAlternateScreen, Hide)?;
        Ok(Self)
    }
}

impl Drop for ScreenGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, Show, LeaveAlternateScreen, ResetColor);
    }
}

const KEYBOARD_FLAGS: KeyboardEnhancementFlags = KeyboardEnhancementFlags::from_bits_retain(
    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES.bits()
        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES.bits()
        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES.bits(),
);

#[derive(Clone, Copy)]
enum InputMode {
    Basic,
    Kitty,
    KittyUnavailable,
}

/// Crossterm adapter. It owns terminal protocol negotiation and produces only terminal events.
struct TerminalInput {
    mode: InputMode,
    kitty_enabled: bool,
}

impl TerminalInput {
    fn basic() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self {
            mode: InputMode::Basic,
            kitty_enabled: false,
        })
    }

    fn automatic() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let kitty_enabled = terminal::supports_keyboard_enhancement().unwrap_or(false);
        if kitty_enabled {
            execute!(io::stdout(), PushKeyboardEnhancementFlags(KEYBOARD_FLAGS))?;
        }
        Ok(Self {
            mode: if kitty_enabled {
                InputMode::Kitty
            } else {
                InputMode::KittyUnavailable
            },
            kitty_enabled,
        })
    }

    fn mode(&self) -> InputMode {
        self.mode
    }

    fn poll(&self, timeout: Duration) -> io::Result<bool> {
        event::poll(timeout)
    }

    fn read_key(&self) -> io::Result<KeyEvent> {
        loop {
            if let Event::Key(key) = event::read()? {
                return Ok(key);
            }
        }
    }
}

impl Drop for TerminalInput {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        if self.kitty_enabled {
            let _ = execute!(stdout, PopKeyboardEnhancementFlags);
        }
        let _ = terminal::disable_raw_mode();
    }
}

#[derive(Default)]
struct KeyTracker {
    down: HashSet<String>,
    seen: HashSet<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeyVisual {
    Never,
    Seen,
    Down,
}

impl KeyTracker {
    fn press(&mut self, id: &str) {
        self.seen.insert(id.to_owned());
        self.down.insert(id.to_owned());
    }

    fn release(&mut self, id: &str) {
        self.seen.insert(id.to_owned());
        self.down.remove(id);
    }

    fn visual(&self, id: &str) -> KeyVisual {
        if self.down.contains(id) {
            KeyVisual::Down
        } else if self.seen.contains(id) {
            KeyVisual::Seen
        } else {
            KeyVisual::Never
        }
    }
}

fn main() -> io::Result<()> {
    let use_basic_input = std::env::args()
        .skip(1)
        .any(|argument| argument == "--no-keyboard-enhancement");
    let input = if use_basic_input {
        TerminalInput::basic()?
    } else {
        TerminalInput::automatic()?
    };
    let input_mode = input.mode();
    let mut stdout = io::stdout();
    let _screen = ScreenGuard::enter(&mut stdout)?;
    let mut mac_keys = KeyTracker::default();
    let mut xt_keys = KeyTracker::default();
    let mut keyboard = PcKeyboard::new(KeyboardModel::XtSet1);
    let mut release_deadlines = HashMap::<String, Instant>::new();
    let mut last = "Waiting for a key event".to_owned();
    let mut modifiers = KeyModifiers::NONE;

    loop {
        if release_expired_keys(
            &mut release_deadlines,
            &mut mac_keys,
            &mut keyboard,
            &mut xt_keys,
        ) {
            modifiers = KeyModifiers::NONE;
        }

        draw(
            &mut stdout,
            &mac_keys,
            &xt_keys,
            modifiers,
            input_mode,
            &last,
        )?;

        if !input.poll(Duration::from_millis(40))? {
            continue;
        }

        let key = input.read_key()?;
        if is_quit(&key) {
            break;
        }
        let Some(input_event) = adapt_key(&key) else {
            last = format!(
                "{:?}  code={}  → unsupported terminal key",
                key.kind, key.code
            );
            continue;
        };

        let id = xt::source_id(input_event.key);
        let mapping = xt::map_key(&input_event, keyboard.model());
        modifiers = key.modifiers;
        last = describe_event(&key, &mapping);

        match input_event.kind {
            xt::InputKind::Press => {
                mac_keys.press(&id);
                apply_pc_events(keyboard.handle(&input_event), &mut xt_keys);
                release_deadlines.insert(id, Instant::now() + SIMULATED_RELEASE_DELAY);
            }
            xt::InputKind::Repeat => {
                mac_keys.press(&id);
                release_deadlines.insert(id, Instant::now() + SIMULATED_RELEASE_DELAY);
            }
            xt::InputKind::Release => {
                release_deadlines.remove(&id);
                mac_keys.release(&id);
                apply_pc_events(keyboard.handle(&input_event), &mut xt_keys);
                modifiers = KeyModifiers::NONE;
            }
        }
    }

    Ok(())
}

fn is_quit(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
        && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn adapt_key(key: &KeyEvent) -> Option<xt::InputEvent> {
    let input_key = match key.code {
        KeyCode::Backspace => xt::InputKey::Backspace,
        KeyCode::Delete => xt::InputKey::Delete,
        KeyCode::Insert => xt::InputKey::Insert,
        KeyCode::Enter => xt::InputKey::Enter,
        KeyCode::Left => xt::InputKey::Left,
        KeyCode::Right => xt::InputKey::Right,
        KeyCode::Up => xt::InputKey::Up,
        KeyCode::Down => xt::InputKey::Down,
        KeyCode::Home => xt::InputKey::Home,
        KeyCode::End => xt::InputKey::End,
        KeyCode::PageUp => xt::InputKey::PageUp,
        KeyCode::PageDown => xt::InputKey::PageDown,
        KeyCode::PrintScreen => xt::InputKey::PrintScreen,
        KeyCode::ScrollLock => xt::InputKey::ScrollLock,
        KeyCode::NumLock => xt::InputKey::NumLock,
        KeyCode::KeypadBegin => xt::InputKey::KeypadBegin,
        KeyCode::Esc => xt::InputKey::Escape,
        KeyCode::Null => xt::InputKey::Null,
        KeyCode::F(number) => xt::InputKey::Function(number),
        KeyCode::Char(character) => xt::InputKey::Char(character),
        KeyCode::Modifier(modifier) => xt::InputKey::Modifier(match modifier {
            ModifierKeyCode::LeftShift => xt::ModifierKey::LeftShift,
            ModifierKeyCode::RightShift => xt::ModifierKey::RightShift,
            ModifierKeyCode::LeftControl => xt::ModifierKey::LeftControl,
            ModifierKeyCode::RightControl => xt::ModifierKey::RightControl,
            ModifierKeyCode::LeftAlt => xt::ModifierKey::LeftAlt,
            ModifierKeyCode::RightAlt => xt::ModifierKey::RightAlt,
            ModifierKeyCode::LeftSuper => xt::ModifierKey::LeftSuper,
            ModifierKeyCode::RightSuper => xt::ModifierKey::RightSuper,
            _ => xt::ModifierKey::Other,
        }),
        _ => return None,
    };
    let mut modifiers = xt::Modifiers::NONE;
    for (active, modifier) in [
        (
            key.modifiers.contains(KeyModifiers::SHIFT),
            xt::Modifiers::SHIFT,
        ),
        (
            key.modifiers.contains(KeyModifiers::CONTROL),
            xt::Modifiers::CONTROL,
        ),
        (
            key.modifiers.contains(KeyModifiers::ALT),
            xt::Modifiers::ALT,
        ),
        (
            key.modifiers.contains(KeyModifiers::SUPER),
            xt::Modifiers::SUPER,
        ),
    ] {
        if active {
            modifiers |= modifier;
        }
    }
    let kind = match key.kind {
        KeyEventKind::Press => xt::InputKind::Press,
        KeyEventKind::Repeat => xt::InputKind::Repeat,
        KeyEventKind::Release => xt::InputKind::Release,
    };
    Some(xt::InputEvent::new(input_key, modifiers, kind))
}

fn release_expired_keys(
    deadlines: &mut HashMap<String, Instant>,
    mac_keys: &mut KeyTracker,
    keyboard: &mut PcKeyboard,
    xt_keys: &mut KeyTracker,
) -> bool {
    let now = Instant::now();
    let expired = deadlines
        .iter()
        .filter(|(_, deadline)| **deadline <= now)
        .map(|(source, _)| source.clone())
        .collect::<Vec<_>>();

    for source in &expired {
        deadlines.remove(source);
        mac_keys.release(source);
        apply_pc_events(keyboard.release_source(source), xt_keys);
    }
    !expired.is_empty()
}

fn apply_pc_events(events: Vec<PcEvent>, keys: &mut KeyTracker) {
    for event in events {
        match event {
            PcEvent::Make(key) => keys.press(key.id),
            PcEvent::Break(key) => keys.release(key.id),
        }
    }
}

fn describe_event(key: &KeyEvent, mapped: &[xt::PcKey]) -> String {
    let modifier_text = if key.modifiers.is_empty() {
        "none".to_owned()
    } else {
        key.modifiers.to_string()
    };
    let pc_text = if mapped.is_empty() {
        "no PC mapping".to_owned()
    } else {
        mapped
            .iter()
            .map(|key| {
                format!(
                    "{}={:02X?}/{:02X?}",
                    key.id,
                    key.make.bytes(),
                    key.break_sequence.bytes()
                )
            })
            .collect::<Vec<_>>()
            .join("  ")
    };

    format!(
        "{:?}  code={}  modifiers={}  state={:?}  → {}",
        key.kind, key.code, modifier_text, key.state, pc_text
    )
}

fn draw(
    stdout: &mut Stdout,
    mac_keys: &KeyTracker,
    xt_keys: &KeyTracker,
    modifiers: KeyModifiers,
    input_mode: InputMode,
    last: &str,
) -> io::Result<()> {
    let (width, height) = terminal::size()?;
    queue!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;

    if width < 132 || height < 31 {
        queue!(
            stdout,
            MoveTo(2, 2),
            Print(format!(
                "Resize terminal to at least 132x31 (currently {}x{}). Ctrl-Q exits.",
                width, height
            ))
        )?;
        stdout.flush()?;
        return Ok(());
    }
    let (protocol, protocol_color) = match input_mode {
        InputMode::Kitty => (
            "Kitty keyboard protocol active: release and modifier events requested.",
            Color::Green,
        ),
        InputMode::Basic => (
            "Basic terminal input: press events use simulated releases.",
            Color::Yellow,
        ),
        InputMode::KittyUnavailable => (
            "Kitty keyboard protocol unavailable: using basic input and simulated releases.",
            Color::Red,
        ),
    };

    queue!(
        stdout,
        MoveTo(2, 0),
        SetForegroundColor(Color::Cyan),
        Print("MacBook Pro → IBM PC/XT Model F keyboard tester"),
        ResetColor,
        MoveTo(2, 1),
        SetForegroundColor(protocol_color),
        Print(protocol),
        ResetColor,
        MoveTo(2, 2),
        Print(
            "Grey = never pressed   Blue = released/simulated release   Green = down now   Ctrl-Q exits."
        ),
        MoveTo(2, 3),
        SetForegroundColor(Color::Yellow),
        Print(trim_to_width(last, width.saturating_sub(4) as usize)),
        ResetColor,
        MoveTo(2, 4),
        Print("MacBook Pro (ANSI U.S.)")
    )?;

    draw_mac(stdout, mac_keys, modifiers)?;
    queue!(
        stdout,
        MoveTo(2, 17),
        Print("Virtual IBM PC/XT 83-key Model F — mapped key state")
    )?;
    draw_xt(stdout, xt_keys)?;

    queue!(
        stdout,
        MoveTo(2, 30),
        SetForegroundColor(Color::DarkGrey),
        Print(
            "Source grid shows physical MacBook keys. F13–F21, navigation, and keypad output exist only on the virtual XT."
        ),
        ResetColor
    )?;
    stdout.flush()
}

fn trim_to_width(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

fn draw_mac(stdout: &mut Stdout, keys: &KeyTracker, modifiers: KeyModifiers) -> io::Result<()> {
    let mut x = 2;
    for (label, width) in [
        ("Esc", 6),
        ("F1", 5),
        ("F2", 5),
        ("F3", 5),
        ("F4", 5),
        ("F5", 5),
        ("F6", 5),
        ("F7", 5),
        ("F8", 5),
        ("F9", 5),
        ("F10", 5),
        ("F11", 5),
        ("F12", 5),
    ] {
        draw_key(
            stdout,
            x,
            5,
            width,
            label,
            mac_visual(keys, modifiers, label),
        )?;
        x += width + 1;
    }

    draw_mac_row(
        stdout,
        7,
        &[
            ("`", 5),
            ("1", 5),
            ("2", 5),
            ("3", 5),
            ("4", 5),
            ("5", 5),
            ("6", 5),
            ("7", 5),
            ("8", 5),
            ("9", 5),
            ("0", 5),
            ("-", 5),
            ("=", 5),
            ("Backspace", 11),
        ],
        keys,
        modifiers,
    )?;
    draw_mac_row(
        stdout,
        9,
        &[
            ("Tab", 8),
            ("Q", 5),
            ("W", 5),
            ("E", 5),
            ("R", 5),
            ("T", 5),
            ("Y", 5),
            ("U", 5),
            ("I", 5),
            ("O", 5),
            ("P", 5),
            ("[", 5),
            ("]", 5),
            ("\\", 5),
        ],
        keys,
        modifiers,
    )?;
    draw_mac_row(
        stdout,
        11,
        &[
            ("Caps", 9),
            ("A", 5),
            ("S", 5),
            ("D", 5),
            ("F", 5),
            ("G", 5),
            ("H", 5),
            ("J", 5),
            ("K", 5),
            ("L", 5),
            (";", 5),
            ("'", 5),
            ("Return", 10),
        ],
        keys,
        modifiers,
    )?;
    draw_mac_row(
        stdout,
        13,
        &[
            ("L Shift", 11),
            ("Z", 5),
            ("X", 5),
            ("C", 5),
            ("V", 5),
            ("B", 5),
            ("N", 5),
            ("M", 5),
            (",", 5),
            (".", 5),
            ("/", 5),
            ("R Shift", 11),
        ],
        keys,
        modifiers,
    )?;
    draw_mac_row(
        stdout,
        15,
        &[
            ("Fn", 5),
            ("Ctrl", 7),
            ("LOpt", 7),
            ("LCmd", 7),
            ("Space", 28),
            ("RCmd", 7),
            ("ROpt", 7),
            ("←", 5),
            ("↑", 5),
            ("↓", 5),
            ("→", 5),
        ],
        keys,
        modifiers,
    )
}

fn draw_xt(stdout: &mut Stdout, keys: &KeyTracker) -> io::Result<()> {
    draw_xt_row(
        stdout,
        19,
        keys,
        ("F1", "F6", "Esc", "ESC", 5, 21),
        &[
            ("1", 4),
            ("2", 4),
            ("3", 4),
            ("4", 4),
            ("5", 4),
            ("6", 4),
            ("7", 4),
            ("8", 4),
            ("9", 4),
            ("0", 4),
            ("-", 4),
            ("=", 4),
            ("Backspace", 10),
        ],
        &[("NumLock", 8), ("Scroll", 7)],
    )?;
    draw_xt_row(
        stdout,
        21,
        keys,
        ("F2", "F7", "Tab", "TAB", 7, 23),
        &[
            ("Q", 4),
            ("W", 4),
            ("E", 4),
            ("R", 4),
            ("T", 4),
            ("Y", 4),
            ("U", 4),
            ("I", 4),
            ("O", 4),
            ("P", 4),
            ("[", 4),
            ("]", 4),
            ("\\", 4),
        ],
        &[("7Home", 7), ("8Up", 6), ("9PgUp", 7), ("KP -", 4)],
    )?;
    draw_xt_row(
        stdout,
        23,
        keys,
        ("F3", "F8", "Ctrl", "CTRL", 7, 23),
        &[
            ("A", 4),
            ("S", 4),
            ("D", 4),
            ("F", 4),
            ("G", 4),
            ("H", 4),
            ("J", 4),
            ("K", 4),
            ("L", 4),
            (";", 4),
            ("'", 4),
            ("Enter", 8),
        ],
        &[("4Left", 7), ("5", 4), ("6Right", 8), ("KP +", 4)],
    )?;
    draw_xt_row(
        stdout,
        25,
        keys,
        ("F4", "F9", "L Shift", "L_SHIFT", 10, 26),
        &[
            ("Z", 4),
            ("X", 4),
            ("C", 4),
            ("V", 4),
            ("B", 4),
            ("N", 4),
            ("M", 4),
            (",", 4),
            (".", 4),
            ("/", 4),
            ("R Shift", 10),
        ],
        &[("1End", 6), ("2Down", 7), ("3PgDn", 7)],
    )?;
    draw_xt_row(
        stdout,
        27,
        keys,
        ("F5", "F10", "Alt", "ALT", 5, 21),
        &[("Space", 38)],
        &[("*Prt", 6), ("0Ins", 5), (".Del", 5)],
    )
}

fn draw_xt_row(
    stdout: &mut Stdout,
    y: u16,
    keys: &KeyTracker,
    prefix: (&str, &str, &str, &str, u16, u16),
    main: &[(&str, u16)],
    keypad: &[(&str, u16)],
) -> io::Result<()> {
    let (left_a, left_b, prefix_label, prefix_id, prefix_width, main_x) = prefix;
    draw_key(stdout, 2, y, 5, left_a, keys.visual(left_a))?;
    draw_key(stdout, 8, y, 5, left_b, keys.visual(left_b))?;
    draw_key(
        stdout,
        15,
        y,
        prefix_width,
        prefix_label,
        keys.visual(prefix_id),
    )?;
    draw_key_group(stdout, main_x, y, main, keys, xt_main_id)?;
    draw_key_group(stdout, 103, y, keypad, keys, xt_keypad_id)
}

fn draw_key_group(
    stdout: &mut Stdout,
    mut x: u16,
    y: u16,
    row: &[(&str, u16)],
    keys: &KeyTracker,
    id: fn(&str) -> &str,
) -> io::Result<()> {
    for &(label, width) in row {
        draw_key(stdout, x, y, width, label, keys.visual(id(label)))?;
        x += width + 1;
    }
    Ok(())
}

fn xt_main_id(label: &str) -> &str {
    match label {
        "Esc" => "ESC",
        "Backspace" => "BACKSPACE",
        "Enter" => "ENTER",
        "Space" => "SPACE",
        other => other,
    }
}

fn xt_keypad_id(label: &str) -> &str {
    match label {
        "NumLock" => "NUMLOCK",
        "Scroll" => "SCROLL",
        "7Home" => "KP_7",
        "8Up" => "KP_8",
        "9PgUp" => "KP_9",
        "KP -" => "KP_MINUS",
        "4Left" => "KP_4",
        "6Right" => "KP_6",
        "KP +" => "KP_PLUS",
        "1End" => "KP_1",
        "2Down" => "KP_2",
        "3PgDn" => "KP_3",
        "*Prt" => "PRINT",
        "0Ins" => "KP_INS",
        ".Del" => "KP_DEL",
        other => other,
    }
}

fn draw_mac_row(
    stdout: &mut Stdout,
    y: u16,
    row: &[(&str, u16)],
    keys: &KeyTracker,
    modifiers: KeyModifiers,
) -> io::Result<()> {
    let mut x = 2;
    for &(label, width) in row {
        draw_key(
            stdout,
            x,
            y,
            width,
            label,
            mac_visual(keys, modifiers, label),
        )?;
        x += width + 1;
    }
    Ok(())
}

fn mac_visual(keys: &KeyTracker, modifiers: KeyModifiers, label: &str) -> KeyVisual {
    let (id, active_modifier) = match label {
        "Esc" => ("ESC", false),
        "Backspace" => ("BACKSPACE", false),
        "Tab" => ("TAB", false),
        "Caps" => ("CAPS", false),
        "Return" => ("RETURN", false),
        "Space" => ("SPACE", false),
        "L Shift" => ("L_SHIFT", modifiers.contains(KeyModifiers::SHIFT)),
        "R Shift" => ("R_SHIFT", modifiers.contains(KeyModifiers::SHIFT)),
        "Ctrl" => ("L_CTRL", modifiers.contains(KeyModifiers::CONTROL)),
        "LOpt" => ("L_OPT", modifiers.contains(KeyModifiers::ALT)),
        "ROpt" => ("R_OPT", modifiers.contains(KeyModifiers::ALT)),
        "LCmd" => ("L_CMD", modifiers.contains(KeyModifiers::SUPER)),
        "RCmd" => ("R_CMD", modifiers.contains(KeyModifiers::SUPER)),
        "←" => ("LEFT", false),
        "→" => ("RIGHT", false),
        "↑" => ("UP", false),
        "↓" => ("DOWN", false),
        other => (other, false),
    };
    if active_modifier {
        return KeyVisual::Down;
    }

    match label {
        "Backspace" => mac_visual_for_ids(keys, &["BACKSPACE", "FORWARD_DELETE"]),
        "←" => mac_visual_for_ids(keys, &["LEFT", "HOME"]),
        "→" => mac_visual_for_ids(keys, &["RIGHT", "END"]),
        "↑" => mac_visual_for_ids(keys, &["UP", "PGUP"]),
        "↓" => mac_visual_for_ids(keys, &["DOWN", "PGDN"]),
        _ => keys.visual(id),
    }
}

fn mac_visual_for_ids(keys: &KeyTracker, ids: &[&str]) -> KeyVisual {
    let mut visual = KeyVisual::Never;
    for id in ids {
        match keys.visual(id) {
            KeyVisual::Down => return KeyVisual::Down,
            KeyVisual::Seen => visual = KeyVisual::Seen,
            KeyVisual::Never => {}
        }
    }
    visual
}

fn draw_key(
    stdout: &mut Stdout,
    x: u16,
    y: u16,
    width: u16,
    label: &str,
    visual: KeyVisual,
) -> io::Result<()> {
    let interior = width.saturating_sub(2) as usize;
    let mut label = trim_to_width(label, interior);
    let padding = interior.saturating_sub(label.chars().count());
    let left = padding / 2;
    label = format!(
        "{}{}{}",
        " ".repeat(left),
        label,
        " ".repeat(padding - left)
    );

    let (background, foreground) = match visual {
        KeyVisual::Never => (Color::DarkGrey, Color::White),
        KeyVisual::Seen => (Color::DarkBlue, Color::White),
        KeyVisual::Down => (Color::Green, Color::Black),
    };
    queue!(
        stdout,
        MoveTo(x, y),
        SetBackgroundColor(background),
        SetForegroundColor(foreground),
        Print(format!("[{}]", label)),
        ResetColor
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapts_crossterm_events_without_exposing_them_to_the_mapper() {
        let terminal_event = KeyEvent::new_with_kind(
            KeyCode::Char('a'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
            KeyEventKind::Press,
        );

        assert_eq!(
            adapt_key(&terminal_event),
            Some(xt::InputEvent::new(
                xt::InputKey::Char('a'),
                xt::Modifiers::CONTROL | xt::Modifiers::ALT,
                xt::InputKind::Press,
            ))
        );
    }

    #[test]
    fn releases_pressed_keys_when_a_terminal_release_is_missing() {
        let input = xt::InputEvent::new(
            xt::InputKey::Enter,
            xt::Modifiers::SUPER,
            xt::InputKind::Press,
        );
        let mut keyboard = PcKeyboard::new(KeyboardModel::XtSet1);
        let mut mac_keys = KeyTracker::default();
        let mut xt_keys = KeyTracker::default();
        let mut deadlines = HashMap::new();

        mac_keys.press("RETURN");
        apply_pc_events(keyboard.handle(&input), &mut xt_keys);
        deadlines.insert(
            "RETURN".to_owned(),
            Instant::now() - Duration::from_millis(1),
        );

        assert!(release_expired_keys(
            &mut deadlines,
            &mut mac_keys,
            &mut keyboard,
            &mut xt_keys,
        ));
        assert_eq!(mac_keys.visual("RETURN"), KeyVisual::Seen);
        assert_eq!(xt_keys.visual("KP_INS"), KeyVisual::Seen);
    }

    #[test]
    fn tracks_never_down_and_released_key_states() {
        let mut keys = KeyTracker::default();

        assert_eq!(keys.visual("A"), KeyVisual::Never);
        keys.press("A");
        assert_eq!(keys.visual("A"), KeyVisual::Down);
        keys.release("A");
        assert_eq!(keys.visual("A"), KeyVisual::Seen);
    }
}
