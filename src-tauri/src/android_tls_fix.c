#include <stdint.h>

// Force 64-byte alignment for the TLS segment to satisfy Android Bionic's linker requirements.
__thread uint64_t _tls_align_fix __attribute__((aligned(64))) = 0;

void ghostty_android_tls_fix() {
    _tls_align_fix++;
}
