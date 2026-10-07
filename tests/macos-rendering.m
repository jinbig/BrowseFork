#import "../src-tauri/src/platform/macos.m"

// Own offscreen AppKit/WebKit windows only: never activate another app, inspect
// the user's screen, open a real browser, or read production preferences.
@interface ProbeApplication : NSApplication
@end
@implementation ProbeApplication
- (void)activateIgnoringOtherApps:(BOOL)flag {}
@end
@interface ProbeWindow : NSWindow
@property NSUInteger paints;
@end
@implementation ProbeWindow
- (void)makeKeyAndOrderFront:(id)sender { [self orderBack:sender]; }
- (void)displayIfNeeded { [super displayIfNeeded]; self.paints += 1; }
@end
static bool current(uint64_t revision) { return revision == 1; }
static void require(BOOL condition, NSString *message) {
    if (!condition) { fprintf(stderr, "%s\n", message.UTF8String); exit(1); }
}
static void spinUntil(BOOL (^condition)(void), NSString *label) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:6.0];
    while (!condition() && deadline.timeIntervalSinceNow > 0)
        [[NSRunLoop currentRunLoop] runMode:NSDefaultRunLoopMode beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
    require(condition(), label);
}
static void checkButtons(NSWindow *window) {
    for (NSNumber *kind in @[@(NSWindowCloseButton), @(NSWindowMiniaturizeButton), @(NSWindowZoomButton)]) {
        NSButton *button = [window standardWindowButton:kind.unsignedIntegerValue];
        require(button != nil && !button.hidden && button.enabled, @"Native Settings button is absent or disabled");
        for (NSView *parent = button; parent; parent = parent.superview)
            require(!parent.hidden && parent.alphaValue > 0.01, @"Native button is concealed by an invisible ancestor");
        NSBitmapImageRep *bitmap = [button bitmapImageRepForCachingDisplayInRect:button.bounds];
        require(bitmap != nil, @"Native button did not produce a bitmap");
        [button cacheDisplayInRect:button.bounds toBitmapImageRep:bitmap];
        CGFloat minimum = 1.0, maximum = 0.0;
        for (NSInteger y = 0; y < bitmap.pixelsHigh; y++) for (NSInteger x = 0; x < bitmap.pixelsWide; x++) {
            NSColor *color = [[bitmap colorAtX:x y:y] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
            CGFloat value = (color.redComponent + color.greenComponent + color.blueComponent + color.alphaComponent) / 4.0;
            minimum = MIN(minimum, value); maximum = MAX(maximum, value);
        }
        require(maximum - minimum > 0.03, @"Native button bitmap is blank; a clickable hit area is not enough");
    }
}
int main(void) {
    @autoreleasepool {
        [ProbeApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
        [NSApp finishLaunching];
        NSWindowStyleMask mask = NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable;
        ProbeWindow *settings = [[ProbeWindow alloc] initWithContentRect:NSMakeRect(-10000, -10000, 620, 500) styleMask:mask backing:NSBackingStoreBuffered defer:NO];
        settings.releasedWhenClosed = NO;
        WKWebView *webview = [[WKWebView alloc] initWithFrame:settings.contentView.bounds];
        webview.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        [settings.contentView addSubview:webview];
        [webview loadHTMLString:@"<html><body><h1>Rules</h1><div id='rules'></div><script>window.browseforkReadyToPresent=async()=>true;</script></body></html>" baseURL:nil];
        spinUntil(^BOOL { return !webview.loading && webview.URL != nil; }, @"WebKit did not load the native rendering fixture");
        browsefork_present_rendered_window((__bridge void *)settings, (__bridge void *)webview, 1, true, current);
        spinUntil(^BOOL { return settings.paints > 0 && settings.visible; }, @"Native Settings did not complete its first frame");
        checkButtons(settings);
        __block BOOL changed = NO;
        [webview evaluateJavaScript:@"document.getElementById('rules').innerHTML='<p>Added rule</p><p>Updated rule</p>';" completionHandler:^(id value, NSError *error) { require(error == nil, @"Rule update JavaScript failed"); changed = YES; }];
        spinUntil(^BOOL { return changed; }, @"Rule update did not complete");
        NSUInteger before = settings.paints;
        browsefork_refresh_settings_after_render((__bridge void *)settings, (__bridge void *)webview, 1, current);
        spinUntil(^BOOL { return settings.paints > before; }, @"Native Settings did not repaint after rule changes");
        checkButtons(settings);
        require(settings.styleMask == mask, @"Rule updates changed the Settings window decorations");
        [settings orderOut:nil];
        puts("Real AppKit title-bar pixels passed before and after a WebKit rule update");
    }
}
