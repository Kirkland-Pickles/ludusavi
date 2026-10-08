#include "../src/gui/macos_reopen.m"
#include <assert.h>

@interface ReopenTestDelegate : NSObject <NSApplicationDelegate>
@end
@implementation ReopenTestDelegate
@end

@interface ExistingReopenDelegate : NSObject <NSApplicationDelegate>
@end
@implementation ExistingReopenDelegate
- (BOOL)applicationShouldHandleReopen:(NSApplication *)application hasVisibleWindows:(BOOL)visible {
    (void)application;
    return visible;
}
@end

int main(void) {
    @autoreleasepool {
        NSApplication *application = [NSApplication sharedApplication];
        id<NSApplicationDelegate> delegate = [ReopenTestDelegate new];
        [application setDelegate:delegate];
        assert(ludusavi_install_reopen_handler());
        assert(ludusavi_install_reopen_handler());
        ludusavi_set_reopen_enabled(true);
        assert(!ludusavi_take_reopen_request());
        assert(![delegate applicationShouldHandleReopen:application hasVisibleWindows:NO]);
        assert(ludusavi_take_reopen_request());
        assert(!ludusavi_take_reopen_request());
        assert(![delegate applicationShouldHandleReopen:application hasVisibleWindows:YES]);
        assert(ludusavi_take_reopen_request());
        ludusavi_set_reopen_enabled(false);
        assert([delegate applicationShouldHandleReopen:application hasVisibleWindows:NO]);
        assert(!ludusavi_take_reopen_request());
        id<NSApplicationDelegate> existing = [ExistingReopenDelegate new];
        [application setDelegate:existing];
        assert(!ludusavi_install_reopen_handler());
        assert([existing applicationShouldHandleReopen:application hasVisibleWindows:YES]);
        assert(!ludusavi_take_reopen_request());
        puts("Native reopen callback and existing delegate preservation passed");
        [application setDelegate:nil];
    }
}
