const std = @import("std");

// Force 64-byte alignment for the TLS segment to satisfy Android Bionic's linker requirements.
threadlocal var _tls_align_fix: u64 align(64) = 0;

export fn ghostty_android_tls_fix() void {
    _tls_align_fix += 1;
}
