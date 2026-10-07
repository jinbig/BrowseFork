#import "../src-tauri/src/platform/macos.m"

// Test the native reveal protocol without opening a window or activating any app.
@interface TestWindow : NSObject
@property(nonatomic, getter=isVisible) BOOL visible;
@property(nonatomic) CGFloat alphaValue;
@property(nonatomic) NSWindowAnimationBehavior animationBehavior;
@property(nonatomic) CGFloat alphaWhenOrderedFront;
@property(nonatomic) NSWindowStyleMask styleMask;
@property(nonatomic) NSWindowTitleVisibility titleVisibility;
@property(nonatomic) BOOL titlebarAppearsTransparent;
@property(nonatomic, strong) NSArray *buttons;
@property(nonatomic) BOOL displayed;
@property(nonatomic) NSUInteger activations;
@property(nonatomic) NSUInteger focusChanges;
@property(nonatomic, strong) NSView *contentView;
@end
@implementation TestWindow
- (void)makeKeyAndOrderFront:(id)sender { self.alphaWhenOrderedFront = self.alphaValue; self.visible = YES; self.activations += 1; }
- (BOOL)makeFirstResponder:(id)responder { self.focusChanges += 1; return YES; }
- (id)standardWindowButton:(NSWindowButton)kind { return self.buttons[kind]; }
- (void)displayIfNeeded { self.displayed = YES; }
@end

@interface TestButton : NSObject
@property(nonatomic) BOOL hidden;
@property(nonatomic) CGFloat alphaValue;
@property(nonatomic) BOOL needsDisplay;
@property(nonatomic, strong) NSView *superview;
@end
@implementation TestButton
@end

@interface TestWebView : NSObject
@property(nonatomic, copy) void (^prepared)(id, NSError *);
@property(nonatomic, copy) void (^painted)(NSImage *, NSError *);
@property(nonatomic) BOOL afterScreenUpdates;
@end
@implementation TestWebView
- (void)callAsyncJavaScript:(NSString *)script arguments:(NSDictionary *)args inFrame:(WKFrameInfo *)frame
    inContentWorld:(WKContentWorld *)world completionHandler:(void (^)(id, NSError *))completion {
    self.prepared = completion;
}
- (void)takeSnapshotWithConfiguration:(WKSnapshotConfiguration *)configuration
    completionHandler:(void (^)(NSImage *, NSError *))completion {
    self.afterScreenUpdates = configuration.afterScreenUpdates;
    self.painted = completion;
}
@end

static uint64_t currentRevision = 1;
static bool requested = true;
static bool isCurrent(uint64_t revision) { return requested && revision == currentRevision; }
static void check(BOOL condition, NSString *message) {
    if (!condition) { fprintf(stderr, "%s\n", message.UTF8String); exit(1); }
}
static void begin(TestWindow *window, TestWebView *webview, uint64_t revision) {
    browsefork_present_rendered_window((__bridge void *)window, (__bridge void *)webview, revision, false, isCurrent);
}
int main(void) {
    @autoreleasepool {
        TestWindow *window = [TestWindow new];
        window.alphaValue = 1.0;
        TestWebView *webview = [TestWebView new];
        begin(window, webview, 1);
        check(window.alphaWhenOrderedFront == 0.0 && window.alphaValue == 0.0, @"The old compositor frame must remain invisible on activation");
        check(webview.prepared && !webview.painted, @"Paint must wait until DOM and assets are ready");
        webview.prepared(@YES, nil);
        check(webview.afterScreenUpdates && window.alphaValue == 0.0, @"Pending screen updates must finish before revealing the window");
        webview.painted([NSImage new], nil);
        check(window.alphaValue == 1.0, @"Only the finished frame may be revealed");

        window.visible = NO;
        begin(window, webview, 1);
        webview.prepared(@YES, nil);
        currentRevision = 2;
        webview.painted([NSImage new], nil);
        check(window.alphaValue == 0.0, @"An obsolete paint callback must not reveal a newer presentation");

        begin(window, webview, 2);
        webview.prepared(@YES, nil);
        requested = false;
        webview.painted([NSImage new], nil);
        check(window.alphaValue == 0.0, @"Closing during preparation must cancel reveal");

        requested = true;
        webview = [TestWebView new];
        begin(window, webview, 2);
        webview.prepared(@NO, nil);
        check(!webview.painted && window.alphaValue == 0.0, @"An invalid DOM revision must not be displayed");

        window.visible = YES;
        window.alphaValue = 1.0;
        begin(window, webview, 2);
        check(window.alphaValue == 1.0, @"Refreshing an already visible chooser must not hide and flash it");
        window.styleMask = NSWindowStyleMaskTitled;
        window.buttons = @[[TestButton new], [TestButton new], [TestButton new]];
        for (TestButton *button in window.buttons) { button.hidden = YES; button.alphaValue = 0.0; }
        window.visible = NO;
        window.alphaValue = 0.0;
        begin(window, webview, 1); // Obsolete request must do nothing.
        browsefork_present_rendered_window((__bridge void *)window, (__bridge void *)webview, 2, true, isCurrent);
        check(!window.visible && window.alphaValue == 1.0, @"Settings must prepare hidden at normal opacity");
        webview.prepared(@YES, nil);
        check(!window.visible, @"Settings must not appear before the frame is painted");
        webview.painted([NSImage new], nil);
        check(window.visible, @"Prepared Settings must be shown normally");
        for (TestButton *button in window.buttons)
            check(!button.hidden && button.alphaValue == 1.0 && button.needsDisplay, @"Settings traffic lights must be visible and repainted");
        check(window.displayed, @"Settings chrome must be painted before the next visible frame");
        NSUInteger activations = window.activations;
        NSUInteger focusChanges = window.focusChanges;
        for (TestButton *button in window.buttons) { button.hidden = YES; button.alphaValue = 0.0; button.needsDisplay = NO; }
        window.displayed = NO;
        browsefork_refresh_settings_after_render((__bridge void *)window, (__bridge void *)webview, 2, isCurrent);
        webview.prepared(@YES, nil);
        check(!window.displayed, @"A Settings update must wait for its completed web frame");
        webview.painted([NSImage new], nil);
        for (TestButton *button in window.buttons)
            check(!button.hidden && button.alphaValue == 1.0 && button.needsDisplay, @"Saving rules must restore Settings traffic lights");
        check(window.displayed && window.alphaValue == 1.0 && window.visible, @"A refresh must repaint chrome without hiding Settings");
        check(window.activations == activations && window.focusChanges == focusChanges, @"A data refresh must not reactivate or steal form focus");
        window.displayed = NO;
        browsefork_refresh_settings_after_render((__bridge void *)window, (__bridge void *)webview, 2, isCurrent);
        webview.prepared(@YES, nil);
        requested = false; // Settings was closed or replaced by the chooser.
        webview.painted([NSImage new], nil);
        check(!window.displayed, @"A stale Settings refresh must not paint another view");
        puts("8 native presentation regression cases passed");
    }
}
