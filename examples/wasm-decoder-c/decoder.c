/*
 * turbohex WASM Decoder Example (C)
 *
 * This is a simple C decoder that can be compiled to WASM.
 * It demonstrates how non-Rust languages can create decoders.
 *
 * Build (requires clang with wasm target):
 *   clang --target=wasm32-unknown-unknown -O2 -nostdlib \
 *     -Wl,--no-entry -Wl,--export-all \
 *     -o color_decoder.wasm decoder.c
 *
 *   cp color_decoder.wasm ~/.config/turbohex/decoders/
 */

/* Simple bump allocator */
static unsigned char heap[65536];
static int heap_offset = 0;

int alloc(int size) {
    int ptr = (int)&heap[heap_offset];
    heap_offset += size;
    if (heap_offset > (int)sizeof(heap)) {
        heap_offset -= size;
        return 0;
    }
    return ptr;
}

/* Helper: write a byte to output buffer */
static int out_pos = 0;
static unsigned char* out_buf = 0;

static void emit(char c) {
    if (out_buf) out_buf[out_pos++] = c;
}

static void emit_str(const char* s) {
    while (*s) emit(*s++);
}

static void emit_hex(unsigned char b) {
    const char* hex = "0123456789ABCDEF";
    emit(hex[b >> 4]);
    emit(hex[b & 0xF]);
}

static void emit_int(int v) {
    if (v == 0) { emit('0'); return; }
    if (v < 0) { emit('-'); v = -v; }
    char buf[12];
    int i = 0;
    while (v > 0) {
        buf[i++] = '0' + (v % 10);
        v /= 10;
    }
    while (--i >= 0) emit(buf[i]);
}

/*
 * Decode: interpret 3-4 bytes as RGB/RGBA color
 */
int decode(int ptr, int len, int endian) {
    unsigned char* bytes = (unsigned char*)ptr;

    /* Allocate output buffer */
    out_buf = (unsigned char*)alloc(4096);
    if (!out_buf) return 0;
    out_pos = 0;

    emit('[');
    int first = 1;

    /* RGB color (3 bytes) */
    if (len >= 3) {
        if (!first) emit(',');
        first = 0;
        emit_str("{\"label\":\"RGB\",\"value\":\"#");
        emit_hex(bytes[0]);
        emit_hex(bytes[1]);
        emit_hex(bytes[2]);
        emit_str("\"}");
    }

    /* RGBA color (4 bytes) */
    if (len >= 4) {
        if (!first) emit(',');
        first = 0;
        emit_str("{\"label\":\"RGBA\",\"value\":\"#");
        emit_hex(bytes[0]);
        emit_hex(bytes[1]);
        emit_hex(bytes[2]);
        emit_hex(bytes[3]);
        emit_str("\"}");

        /* Alpha percentage */
        if (!first) emit(',');
        emit_str("{\"label\":\"Alpha\",\"value\":\"");
        emit_int((int)(bytes[3] * 100 / 255));
        emit_str("%\"}");
    }

    emit(']');
    emit(0); /* NUL terminator */

    return (int)out_buf;
}
