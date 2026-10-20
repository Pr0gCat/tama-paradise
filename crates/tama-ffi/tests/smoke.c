/* Host smoke test of the C ABI (needs the local, unpublished data/game.json):
 *   cargo build -p tama-ffi
 *   cc -Wall -Icrates/tama-ffi/include crates/tama-ffi/tests/smoke.c target/debug/libtama_ffi.a -o target/smoke
 *   target/smoke data/game.json
 */
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "tama.h"

static char *read_file(const char *path) {
    FILE *f = fopen(path, "rb");
    assert(f);
    fseek(f, 0, SEEK_END);
    long n = ftell(f);
    rewind(f);
    char *s = malloc(n + 1);
    assert(fread(s, 1, n, f) == (size_t)n);
    s[n] = 0;
    fclose(f);
    return s;
}

int main(int argc, char **argv) {
    char *err = NULL;
    assert(!tama_new("not json", 1, &err) && err);
    tama_string_free(err);

    assert(argc == 2);
    char *data = read_file(argv[1]);
    TamaGame *g = tama_new(data, 1, &err);
    assert(g);

    size_t n;
    uint16_t *missing = tama_missing_sprites(g, &n);
    printf("missing sprites: %zu\n", n);
    tama_u16_free(missing, n);

    uint64_t now = 1700000000;
    for (int i = 0; i < 100; i++) tama_frame(g, now, i % 10 == 0 ? TAMA_A : 0);
    const uint8_t *fb = tama_framebuffer(g, &n);
    assert(fb && n == TAMA_WIDTH * TAMA_HEIGHT * 4);
    uint32_t *sounds = tama_take_sounds(g, &n);
    printf("sounds: %zu\n", n);
    tama_u32_free(sounds, n);
    char *connect = tama_take_connect(g);
    assert(strcmp(connect, "") == 0);
    tama_string_free(connect);
    tama_set_notify(g, true);
    assert(tama_notify(g));
    tama_set_build(g, "smoke");
    printf("touch %d text_entry %d poll %d\n", tama_touch(g), tama_text_entry(g), tama_poll_call(g, now + 60));

    char *save = tama_save(g);
    tama_free(g);
    g = tama_load(data, save, &err);
    assert(g);
    tama_advance_to(g, now + 3600);
    tama_string_free(save);
    tama_free(g);
    free(data);
    puts("ok");
    return 0;
}
