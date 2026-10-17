/* doomgeneric platform layer for makine. */
#include <stdint.h>
#include <stdio.h>

#include "doomgeneric.h"
#include "doomkeys.h"
#include <makine.h>

extern const char wad_data[];
extern const unsigned wad_size;
void mk_add_file(const char *name, const void *data, size_t size);

void DG_Init(void) {}

void DG_DrawFrame(void) {
    const uint32_t *src = DG_ScreenBuffer;
    volatile uint32_t *dst = MK_SCREEN;
    for (int i = 0; i < DOOMGENERIC_RESX * DOOMGENERIC_RESY; i++) dst[i] = src[i];
    mk_show();
}

void DG_SleepMs(uint32_t ms) {
    uint32_t until = mk_millis() + ms;
    while ((int32_t)(mk_millis() - until) < 0);
}

uint32_t DG_GetTicksMs(void) { return mk_millis(); }

static unsigned char translate(int k) {
    switch (k) {
    case MK_KEY_LEFT: return KEY_LEFTARROW;
    case MK_KEY_RIGHT: return KEY_RIGHTARROW;
    case MK_KEY_UP: return KEY_UPARROW;
    case MK_KEY_DOWN: return KEY_DOWNARROW;
    case MK_KEY_CTRL: return KEY_FIRE;
    case MK_KEY_SHIFT: return KEY_RSHIFT;
    case MK_KEY_ALT: return KEY_RALT;
    case ' ': return KEY_USE;
    case 8: return KEY_BACKSPACE;
    default: return k;
    }
}

int DG_GetKey(int *pressed, unsigned char *key) {
    int k = mk_key(pressed);
    if (!k) return 0;
    *key = translate(k);
    return 1;
}

void DG_SetWindowTitle(const char *title) { (void)title; }

int main(void) {
    mk_add_file("doom1.wad", wad_data, wad_size);
    static char *argv[] = {"doom", "-iwad", "doom1.wad", 0};
    doomgeneric_Create(3, argv);
    for (;;) doomgeneric_Tick();
}
