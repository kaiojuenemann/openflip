// C interface of the openflip-ffi crate (crates/openflip-ffi/src/lib.rs).
// Keep both in sync. All functions must be called on the main thread.

#ifndef OPENFLIP_H
#define OPENFLIP_H

#include <stdbool.h>
#include <stdint.h>

typedef struct OpenFlip OpenFlip;

// Results of the input functions: what the host should do next.
static const uint32_t OPENFLIP_NOTHING = 0;
static const uint32_t OPENFLIP_REDRAW = 1;
static const uint32_t OPENFLIP_CHOOSE_FONT = 2;  // show a font picker, then openflip_set_font
static const uint32_t OPENFLIP_EXIT = 3;         // close the settings window

// Keys for openflip_key.
static const uint32_t OPENFLIP_KEY_UP = 0;
static const uint32_t OPENFLIP_KEY_DOWN = 1;
static const uint32_t OPENFLIP_KEY_LEFT = 2;
static const uint32_t OPENFLIP_KEY_RIGHT = 3;
static const uint32_t OPENFLIP_KEY_ACTIVATE = 4;  // Return, Space
static const uint32_t OPENFLIP_KEY_RESET = 5;     // Delete, Backspace
static const uint32_t OPENFLIP_KEY_SETTINGS = 6;  // S
static const uint32_t OPENFLIP_KEY_ESCAPE = 7;
static const uint32_t OPENFLIP_KEY_QUIT = 8;      // Q

// Creates a clock drawing into an NSView of width x height pixels; null on
// failure. The view must outlive the handle.
OpenFlip *openflip_new_for_view(void *view, uint32_t width, uint32_t height, bool settings_only);
void openflip_free(OpenFlip *handle);

void openflip_resize(OpenFlip *handle, uint32_t width, uint32_t height);
// Draws if a frame is due; cheap otherwise. Returns whether it drew.
bool openflip_draw(OpenFlip *handle);

// Input; coordinates are pixels from the view's top-left corner.
uint32_t openflip_key(OpenFlip *handle, uint32_t key);
uint32_t openflip_mouse_moved(OpenFlip *handle, float x, float y);
uint32_t openflip_mouse_pressed(OpenFlip *handle, float x, float y);
uint32_t openflip_set_font(OpenFlip *handle, const char *path);

#endif
