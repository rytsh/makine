/* A small C program for makine: prints, then draws a moving gradient.
 * Build: ../../sdk/build.sh merhaba.c   ->  merhaba.elf */
#include <stdio.h>
#include <makine.h>

int main(void) {
    printf("Merhaba C! %d + %d = %d\n", 40, 2, 40 + 2);
    for (int t = 0;; t += 4) {
        for (int y = 0; y < MK_SCREEN_H; y++)
            for (int x = 0; x < MK_SCREEN_W; x++)
                MK_SCREEN[y * MK_SCREEN_W + x] = ((x + t) & 255) << 16 | ((y * 2) & 255) << 8 | (t & 255);
        mk_show();
    }
}
