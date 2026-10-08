#import <AppKit/AppKit.h>
#import <objc/runtime.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdio.h>

static atomic_bool reopen_requested = false;
static atomic_bool reopen_enabled = false;

static BOOL handle_reopen(__unused id self, __unused SEL command,
                          __unused NSApplication *application, __unused BOOL visible) {
    if (!atomic_load_explicit(&reopen_enabled, memory_order_relaxed)) return YES;
    atomic_store_explicit(&reopen_requested, true, memory_order_relaxed);
    return NO;
}

static bool install_reopen_handler(Class delegate) {
    if (!delegate) return false;
    SEL selector = @selector(applicationShouldHandleReopen:hasVisibleWindows:);
    Method existing = class_getInstanceMethod(delegate, selector);
    if (existing) return method_getImplementation(existing) == (IMP)handle_reopen;
    char encoding[16];
    snprintf(encoding, sizeof(encoding), "%s@:@%s", @encode(BOOL), @encode(BOOL));
    // Keep Winit's delegate and add only the optional reopen callback.
    return class_addMethod(delegate, selector, (IMP)handle_reopen, encoding);
}

bool ludusavi_install_reopen_handler(void) {
    return install_reopen_handler(object_getClass([NSApp delegate]));
}

bool ludusavi_take_reopen_request(void) {
    return atomic_exchange_explicit(&reopen_requested, false, memory_order_relaxed);
}

void ludusavi_set_reopen_enabled(bool enabled) {
    atomic_store_explicit(&reopen_enabled, enabled, memory_order_relaxed);
    if (!enabled) atomic_store_explicit(&reopen_requested, false, memory_order_relaxed);
}
