/* makine: the machine's devices. See crates/rv/src/machine.rs. */
#ifndef MAKINE_H
#define MAKINE_H

#include <stdint.h>

#define MK_CONSOLE   (*(volatile uint32_t *)0x10000000u)
#define MK_TIMER     (*(volatile uint32_t *)0x10000004u)
#define MK_KEYBOARD  (*(volatile uint32_t *)0x10000008u)
#define MK_SHOW      (*(volatile uint32_t *)0x1000000Cu)
#define MK_SCREEN_W  320
#define MK_SCREEN_H  200
#define MK_RANDOM    (*(volatile uint32_t *)0x10000018u)
#define MK_SCREEN    ((volatile uint32_t *)0x11000000u)

/* Key codes for non-character keys; characters use ASCII. */
#define MK_KEY_LEFT  0x80
#define MK_KEY_RIGHT 0x81
#define MK_KEY_UP    0x82
#define MK_KEY_DOWN  0x83
#define MK_KEY_CTRL  0x84
#define MK_KEY_SHIFT 0x85
#define MK_KEY_ALT   0x86

static inline uint32_t mk_millis(void) { return MK_TIMER; }

/* Next key event: returns 0 if none, else sets *pressed and returns the key. */
static inline int mk_key(int *pressed) {
    uint32_t e = MK_KEYBOARD;
    if (!e) return 0;
    *pressed = (e >> 8) & 1;
    return e & 0xFF;
}

static inline void mk_show(void) { MK_SHOW = 0; }

#endif
