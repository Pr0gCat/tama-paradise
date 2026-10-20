/* C ABI of crates/tama-ffi (hand-written; keep in sync with src/lib.rs). */
#ifndef TAMA_H
#define TAMA_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define TAMA_WIDTH 128
#define TAMA_HEIGHT 128
#define TAMA_TICK_MS 50 /* one frame at 20 fps */

/* Input bits for tama_frame's `held`. */
#define TAMA_A (1u << 0)
#define TAMA_B (1u << 1)
#define TAMA_C (1u << 2)
#define TAMA_DIAL_PUSH (1u << 3)
#define TAMA_DIAL_DOWN (1u << 6)
#define TAMA_DIAL_UP (1u << 7)

/* Opaque game handle. Not thread-safe: use one handle from one thread at a time. */
typedef struct TamaGame TamaGame;

/*
 * Ownership: every `char *` returned is freed with tama_string_free; every array returned with
 * a length is freed with the matching tama_*_free and that length. String arguments are
 * NUL-terminated UTF-8 (NULL or invalid UTF-8 reads as ""). Times are local wall-clock seconds.
 */

/* `data` = contents of data/game.json. Returns NULL on error and, if `err` is not NULL,
 * sets *err to a message (free it). */
TamaGame *tama_new(const char *data, uint32_t seed, char **err);
/* Restore from a tama_save string. Call tama_advance_to afterwards to catch up offline time. */
TamaGame *tama_load(const char *data, const char *save, char **err);
void tama_free(TamaGame *game);

void tama_string_free(char *s);
void tama_u16_free(uint16_t *p, size_t len);
void tama_u32_free(uint32_t *p, size_t len);

char *tama_save(const TamaGame *game);

/* One 20 fps frame. */
void tama_frame(TamaGame *game, uint64_t now, uint8_t held);
/* While in the background: advance the pet and return the request id of a care call that just
 * started (0x0f HOME, 0x11 hunger, 0x12 mood, 0x0a cold, 0x13 evolution support, 0x1c DODODO),
 * or -1 for none. */
int32_t tama_poll_call(TamaGame *game, uint64_t now);
/* Catch-up without input, e.g. after loading. */
void tama_advance_to(TamaGame *game, uint64_t now);

/* RGBA, TAMA_WIDTH x TAMA_HEIGHT, row-major. Borrowed: valid until the next call that takes a
 * non-const game. */
const uint8_t *tama_framebuffer(const TamaGame *game, size_t *len);

/* Sprite indexes to supply with tama_add_sprite. Free with tama_u16_free. */
uint16_t *tama_missing_sprites(const TamaGame *game, size_t *len);
/* `bytes` = decompressed .tsp file. */
bool tama_add_sprite(TamaGame *game, uint16_t id, const uint8_t *bytes, size_t len);

/* Sounds started since the last call, as id << 8 | volume. Free with tama_u32_free. */
uint32_t *tama_take_sounds(TamaGame *game, size_t *len);
/* CONNECTION request: "", "playdate", "receive" or "gift:<code>". Free the string. */
char *tama_take_connect(TamaGame *game);

/* The screen takes typed text; pass keys (KeyboardEvent.key names) to tama_type_key. */
bool tama_text_entry(const TamaGame *game);
bool tama_type_key(TamaGame *game, const char *key);

/* SETTINGS -> TOUCH: swipes and taps turn and press the dial. */
bool tama_touch(const TamaGame *game);
/* SETTINGS -> NOTIFY: care calls become notifications while in the background. */
bool tama_notify(const TamaGame *game);
void tama_set_notify(TamaGame *game, bool on);

/* Build shown in GAME INFO. */
void tama_set_build(TamaGame *game, const char *build);
/* The cold-start DELETE menu (CONTINUE / RESET ALL). */
void tama_begin_delete(TamaGame *game);

#ifdef __cplusplus
}
#endif

#endif
