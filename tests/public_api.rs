use pc_xt_keyboard::{
    InputEvent, InputKey, InputKind, KeyboardModel, Modifiers, PcEvent, PcKeyboard,
};

#[test]
fn consumer_receives_transport_neutral_make_and_break_sequences() {
    let mut keyboard = PcKeyboard::new(KeyboardModel::XtSet1);
    let press = InputEvent::new(InputKey::Char('a'), Modifiers::NONE, InputKind::Press);
    let release = InputEvent::new(InputKey::Char('a'), Modifiers::NONE, InputKind::Release);

    assert!(matches!(
        keyboard.handle(&press).as_slice(),
        [PcEvent::Make(key)] if key.make.bytes() == [0x1E]
    ));
    assert!(matches!(
        keyboard.handle(&release).as_slice(),
        [PcEvent::Break(key)] if key.break_sequence.bytes() == [0x9E]
    ));
}
