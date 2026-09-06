#ifndef PC_XT_KEYBOARD_H
#define PC_XT_KEYBOARD_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define PC_XT_KEYBOARD_V1_EVENT_MAX_BYTES 32u
#define PC_XT_KEYBOARD_V1_ERROR SIZE_MAX

typedef enum {
    PC_XT_KEYBOARD_V1_XT_SET1 = 0,
    PC_XT_KEYBOARD_V1_AT_SET1 = 1,
} pc_xt_keyboard_v1_model;

typedef enum {
    PC_XT_KEYBOARD_V1_PRESS = 0,
    PC_XT_KEYBOARD_V1_REPEAT = 1,
    PC_XT_KEYBOARD_V1_RELEASE = 2,
} pc_xt_keyboard_v1_input_kind;

enum {
    PC_XT_KEYBOARD_V1_MOD_SHIFT = 1u << 0,
    PC_XT_KEYBOARD_V1_MOD_CONTROL = 1u << 1,
    PC_XT_KEYBOARD_V1_MOD_ALT = 1u << 2,
    PC_XT_KEYBOARD_V1_MOD_SUPER = 1u << 3,
};

typedef enum {
    PC_XT_KEYBOARD_V1_KEY_CHAR = 0,
    PC_XT_KEYBOARD_V1_KEY_BACKSPACE = 1,
    PC_XT_KEYBOARD_V1_KEY_DELETE = 2,
    PC_XT_KEYBOARD_V1_KEY_INSERT = 3,
    PC_XT_KEYBOARD_V1_KEY_ENTER = 4,
    PC_XT_KEYBOARD_V1_KEY_LEFT = 5,
    PC_XT_KEYBOARD_V1_KEY_RIGHT = 6,
    PC_XT_KEYBOARD_V1_KEY_UP = 7,
    PC_XT_KEYBOARD_V1_KEY_DOWN = 8,
    PC_XT_KEYBOARD_V1_KEY_HOME = 9,
    PC_XT_KEYBOARD_V1_KEY_END = 10,
    PC_XT_KEYBOARD_V1_KEY_PAGE_UP = 11,
    PC_XT_KEYBOARD_V1_KEY_PAGE_DOWN = 12,
    PC_XT_KEYBOARD_V1_KEY_PRINT_SCREEN = 13,
    PC_XT_KEYBOARD_V1_KEY_PAUSE = 14,
    PC_XT_KEYBOARD_V1_KEY_SCROLL_LOCK = 15,
    PC_XT_KEYBOARD_V1_KEY_NUM_LOCK = 16,
    PC_XT_KEYBOARD_V1_KEY_KEYPAD_BEGIN = 17,
    PC_XT_KEYBOARD_V1_KEY_ESCAPE = 18,
    PC_XT_KEYBOARD_V1_KEY_NULL = 19,
    PC_XT_KEYBOARD_V1_KEY_FUNCTION = 20,
    PC_XT_KEYBOARD_V1_KEY_MODIFIER = 21,
} pc_xt_keyboard_v1_key_kind;

typedef enum {
    PC_XT_KEYBOARD_V1_LEFT_SHIFT = 0,
    PC_XT_KEYBOARD_V1_RIGHT_SHIFT = 1,
    PC_XT_KEYBOARD_V1_LEFT_CONTROL = 2,
    PC_XT_KEYBOARD_V1_RIGHT_CONTROL = 3,
    PC_XT_KEYBOARD_V1_LEFT_ALT = 4,
    PC_XT_KEYBOARD_V1_RIGHT_ALT = 5,
    PC_XT_KEYBOARD_V1_LEFT_SUPER = 6,
    PC_XT_KEYBOARD_V1_RIGHT_SUPER = 7,
    PC_XT_KEYBOARD_V1_OTHER_MODIFIER = 8,
} pc_xt_keyboard_v1_modifier_key;

typedef struct {
    uint32_t kind;
    uint32_t value;
    uint32_t character;
} pc_xt_keyboard_v1_input_key;

typedef struct {
    pc_xt_keyboard_v1_input_key key;
    uint8_t modifiers;
    uint8_t kind;
} pc_xt_keyboard_v1_input_event;

/* Opaque mapper state. Destroy with pc_xt_keyboard_v1_destroy. */
void *pc_xt_keyboard_v1_create(uint32_t model);
void pc_xt_keyboard_v1_destroy(void *keyboard);

/*
 * Processes one host input event. `output` must point to at least
 * PC_XT_KEYBOARD_V1_EVENT_MAX_BYTES bytes. Returns emitted byte count, or
 * PC_XT_KEYBOARD_V1_ERROR for invalid arguments or an undersized buffer.
 */
size_t pc_xt_keyboard_v1_handle(
    void *keyboard,
    const pc_xt_keyboard_v1_input_event *input,
    uint8_t *output,
    size_t output_capacity
);

#ifdef __cplusplus
}
#endif

#endif
