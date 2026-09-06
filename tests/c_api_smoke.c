#include "pc_xt_keyboard.h"

#include <assert.h>
#include <stdint.h>

static size_t handle(void *keyboard, pc_xt_keyboard_v1_input_event input, uint8_t output[PC_XT_KEYBOARD_V1_EVENT_MAX_BYTES]) {
    return pc_xt_keyboard_v1_handle(
        keyboard,
        &input,
        output,
        PC_XT_KEYBOARD_V1_EVENT_MAX_BYTES
    );
}

int main(void) {
    uint8_t output[PC_XT_KEYBOARD_V1_EVENT_MAX_BYTES];
    void *keyboard = pc_xt_keyboard_v1_create(PC_XT_KEYBOARD_V1_AT_SET1);
    assert(keyboard != 0);

    pc_xt_keyboard_v1_input_event a_press = {
        .key = { .kind = PC_XT_KEYBOARD_V1_KEY_CHAR, .character = 'a' },
        .kind = PC_XT_KEYBOARD_V1_PRESS,
    };
    assert(handle(keyboard, a_press, output) == 1 && output[0] == 0x1E);

    pc_xt_keyboard_v1_input_event a_release = a_press;
    a_release.kind = PC_XT_KEYBOARD_V1_RELEASE;
    assert(handle(keyboard, a_release, output) == 1 && output[0] == 0x9E);

    pc_xt_keyboard_v1_input_event print = {
        .key = { .kind = PC_XT_KEYBOARD_V1_KEY_PRINT_SCREEN },
        .kind = PC_XT_KEYBOARD_V1_PRESS,
    };
    assert(handle(keyboard, print, output) == 4);
    assert(output[0] == 0xE0 && output[1] == 0x2A && output[2] == 0xE0 && output[3] == 0x37);

    pc_xt_keyboard_v1_input_event pause = {
        .key = { .kind = PC_XT_KEYBOARD_V1_KEY_CHAR, .character = '\\' },
        .modifiers = PC_XT_KEYBOARD_V1_MOD_CONTROL | PC_XT_KEYBOARD_V1_MOD_SHIFT | PC_XT_KEYBOARD_V1_MOD_ALT | PC_XT_KEYBOARD_V1_MOD_SUPER,
        .kind = PC_XT_KEYBOARD_V1_PRESS,
    };
    assert(handle(keyboard, pause, output) == 6);
    assert(output[0] == 0xE1 && output[1] == 0x1D && output[2] == 0x45 && output[3] == 0xE1 && output[4] == 0x9D && output[5] == 0xC5);

    pc_xt_keyboard_v1_input_event sysrq = pause;
    sysrq.key.character = '`';
    assert(handle(keyboard, sysrq, output) == 1 && output[0] == 0x54);

    pc_xt_keyboard_v1_destroy(keyboard);
    return 0;
}
