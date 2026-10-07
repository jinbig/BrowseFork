#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#import <QuartzCore/QuartzCore.h>
#import <WebKit/WebKit.h>
#import <objc/runtime.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>

// Matches the switcher border radius in ui/styles.css.
static const CGFloat BrowseForkSwitcherCornerRadius = 24.0;

typedef void (*BrowseForkURLCallback)(const char *, const char *);
static BrowseForkURLCallback browseforkURLCallback;
static void (*browseforkAppDeactivatedCallback)(void);
static NSMutableArray<NSDictionary *> *browseforkPendingURLs;
static bool browseforkWasDefaultLaunch;

@interface BrowseForkLaunchObserver : NSObject
@end

@implementation BrowseForkLaunchObserver
- (void)didFinishLaunching:(NSNotification *)notification {
    NSNumber *defaultLaunch = notification.userInfo[@"NSApplicationLaunchIsDefaultLaunchKey"];
    browseforkWasDefaultLaunch = [defaultLaunch boolValue];
}
- (void)didResignActive:(NSNotification *)notification {
    (void)notification;
    if (browseforkAppDeactivatedCallback) browseforkAppDeactivatedCallback();
}
@end

static BrowseForkLaunchObserver *browseforkLaunchObserver;

void browsefork_install_launch_observer(void) {
    browseforkLaunchObserver = [BrowseForkLaunchObserver new];
    [[NSNotificationCenter defaultCenter]
        addObserver:browseforkLaunchObserver
           selector:@selector(didFinishLaunching:)
               name:NSApplicationDidFinishLaunchingNotification
             object:nil];
    [[NSNotificationCenter defaultCenter]
        addObserver:browseforkLaunchObserver
           selector:@selector(didResignActive:)
               name:NSApplicationDidResignActiveNotification
             object:NSApp];
}

bool browsefork_was_default_launch(void) {
    return browseforkWasDefaultLaunch;
}

void browsefork_install_deactivation_handler(void (*callback)(void)) {
    browseforkAppDeactivatedCallback = callback;
}

bool browsefork_is_app_active(void) {
    return [NSApp isActive];
}

void browsefork_style_picker_window(void *native_window, bool switcher) {
    NSWindow *window = (__bridge NSWindow *)native_window;
    if (!window) return;
    NSView *content = window.contentView;
    if (!content) return;
    content.wantsLayer = YES;
    content.layer.cornerRadius = switcher ? BrowseForkSwitcherCornerRadius : 0.0;
    content.layer.masksToBounds = switcher;
    window.opaque = !switcher;
    window.backgroundColor = switcher ? NSColor.clearColor : NSColor.windowBackgroundColor;
    [window invalidateShadow];
}

void browsefork_activate_window(void *native_window) {
    NSWindow *window = (__bridge NSWindow *)native_window;
    if (!window) return;
    [NSApp activateIgnoringOtherApps:YES];
    [window makeKeyAndOrderFront:nil];
}

// AppKit chrome is outside the WKWebView paint checkpoint. Repaint it when
// a titled Settings window is revealed or its web content changes.
static void browsefork_refresh_settings_titlebar(NSWindow *window) {
    if (!(window.styleMask & NSWindowStyleMaskTitled)) return;
    window.titleVisibility = NSWindowTitleVisible;
    window.titlebarAppearsTransparent = NO;
    for (NSNumber *kind in @[@(NSWindowCloseButton), @(NSWindowMiniaturizeButton), @(NSWindowZoomButton)]) {
        NSButton *button = [window standardWindowButton:kind.unsignedIntegerValue];
        button.hidden = NO;
        button.alphaValue = 1.0;
        button.needsDisplay = YES;
        button.superview.needsDisplay = YES;
    }
    window.contentView.superview.needsDisplay = YES;
    [window displayIfNeeded];
}

// A hidden WKWebView can still hold the previous compositor frame. Activate it
// invisibly, finish the DOM/images/fonts, and wait for a rendered snapshot before
// exposing the window. This is a paint barrier, not a timed fade or delay.
void browsefork_present_rendered_window(void *native_window, void *native_webview,
                                        uint64_t revision, bool settings, bool (*is_current)(uint64_t)) {
    NSWindow *window = (__bridge NSWindow *)native_window;
    WKWebView *webview = (__bridge WKWebView *)native_webview;
    if (!window || !webview || !is_current(revision)) return;
    BOOL alreadyVisible = window.visible && window.alphaValue > 0.0;
    if (settings) {
        // Settings chrome must never be constructed or activated at zero opacity.
        // Tauri's decoration change is asynchronous; apply the intended style here
        // before preparing the first frame, rather than relying on its timing.
        window.alphaValue = 1.0;
        NSWindowStyleMask settingsStyle = NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
            NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable;
        if ((window.styleMask & settingsStyle) != settingsStyle) window.styleMask |= settingsStyle;
        window.titleVisibility = NSWindowTitleVisible;
        window.titlebarAppearsTransparent = NO;
    } else if (!alreadyVisible) {
        window.alphaValue = 0.0;
    }
    window.animationBehavior = NSWindowAnimationBehaviorNone;
    if (!settings || alreadyVisible) {
        [NSApp activateIgnoringOtherApps:YES];
        [window makeKeyAndOrderFront:nil];
        [window makeFirstResponder:webview];
    }
    [webview callAsyncJavaScript:@"return await window.browseforkReadyToPresent(revision);"
                       arguments:@{@"revision": @(revision)}
                         inFrame:nil
                  inContentWorld:WKContentWorld.pageWorld
               completionHandler:^(id ready, NSError *error) {
        if (!is_current(revision)) return;
        if (error || ![ready isEqual:@YES]) {
            if (error) NSLog(@"BrowseFork: could not prepare window: %@", error);
            return;
        }
        WKSnapshotConfiguration *configuration = [WKSnapshotConfiguration new];
        configuration.afterScreenUpdates = YES;
        [webview takeSnapshotWithConfiguration:configuration completionHandler:^(NSImage *image, NSError *snapshotError) {
            if (!is_current(revision) || (!settings && !window.visible)) return;
            if (snapshotError || !image) {
                NSLog(@"BrowseFork: could not render window: %@", snapshotError);
                return;
            }
            window.alphaValue = 1.0;
            if (settings && !alreadyVisible) {
                [NSApp activateIgnoringOtherApps:YES];
                [window makeKeyAndOrderFront:nil];
                [window makeFirstResponder:webview];
            }
            browsefork_refresh_settings_titlebar(window);
        }];
    }];
}

// A rule edit changes only web content, so no new window presentation occurs.
// Refresh AppKit chrome after that frame finishes, without resizing, activating,
// or moving keyboard focus away from the Settings form.
void browsefork_refresh_settings_after_render(void *native_window, void *native_webview,
                                              uint64_t revision, bool (*is_current)(uint64_t)) {
    NSWindow *window = (__bridge NSWindow *)native_window;
    WKWebView *webview = (__bridge WKWebView *)native_webview;
    if (!window || !webview || !window.visible || !is_current(revision)) return;
    [webview callAsyncJavaScript:@"return await window.browseforkReadyToPresent(revision);"
                       arguments:@{@"revision": @(revision)}
                         inFrame:nil
                  inContentWorld:WKContentWorld.pageWorld
               completionHandler:^(id ready, NSError *error) {
        if (error || ![ready isEqual:@YES] || !window.visible || !is_current(revision)) return;
        WKSnapshotConfiguration *configuration = [WKSnapshotConfiguration new];
        configuration.afterScreenUpdates = YES;
        [webview takeSnapshotWithConfiguration:configuration completionHandler:^(NSImage *image, NSError *snapshotError) {
            if (snapshotError || !image || !window.visible || !is_current(revision)) return;
            browsefork_refresh_settings_titlebar(window);
        }];
    }];
}

static void browsefork_open_urls(id self, SEL selector, NSApplication *app,
                                 NSArray<NSURL *> *urls) {
    (void)self;
    (void)selector;
    (void)app;
    // The sender is only trustworthy while AppKit is handling this Apple event.
    // Do not infer it from the frontmost application when the attribute is absent.
    NSAppleEventDescriptor *event = [[NSAppleEventManager sharedAppleEventManager] currentAppleEvent];
    NSAppleEventDescriptor *sender = [event attributeDescriptorForKeyword:keySenderPIDAttr];
    pid_t senderPID = sender ? (pid_t)[sender int32Value] : 0;
    NSRunningApplication *source = senderPID > 0
        ? [NSRunningApplication runningApplicationWithProcessIdentifier:senderPID] : nil;
    NSString *sourceID = source.bundleIdentifier;
    for (NSURL *url in urls) {
        NSString *value = [url absoluteString];
        if (!value) continue;
        if (browseforkURLCallback) browseforkURLCallback([value UTF8String], [sourceID UTF8String]);
        else [browseforkPendingURLs addObject:@{ @"url": value, @"source": sourceID ?: [NSNull null] }];
    }
}

bool browsefork_capture_delegate_urls(void) {
    // Tauri installs this delegate during build, before the run callback is ready.
    // Capture early URL events here so a cold launch does not lose the link.
    Class delegate = objc_getClass("TaoAppDelegateParent");
    if (!delegate) return false;
    Method openURLs = class_getInstanceMethod(delegate, @selector(application:openURLs:));
    if (!openURLs) return false;
    if (!browseforkPendingURLs) browseforkPendingURLs = [NSMutableArray new];
    method_setImplementation(openURLs, (IMP)browsefork_open_urls);
    return true;
}

void browsefork_install_url_handler(BrowseForkURLCallback callback) {
    browseforkURLCallback = callback;
    for (NSDictionary *link in browseforkPendingURLs) {
        NSString *source = link[@"source"];
        callback([link[@"url"] UTF8String], source == (id)[NSNull null] ? NULL : [source UTF8String]);
    }
    [browseforkPendingURLs removeAllObjects];
}

static bool browsefork_is_browser(NSBundle *bundle) {
    NSDictionary *info = [bundle infoDictionary];
    NSString *bundleID = [bundle bundleIdentifier];
    if ([bundleID isEqualToString:@"com.browsefork.desktop"] ||
        [bundleID isEqualToString:@"com.choosyosx.Choosy"]) {
        return false;
    }

    bool handlesHttp = false;
    bool handlesHttps = false;
    for (NSDictionary *type in info[@"CFBundleURLTypes"]) {
        NSArray<NSString *> *schemes = type[@"CFBundleURLSchemes"];
        if ([type[@"LSHandlerRank"] isEqualToString:@"Alternate"]) continue;
        handlesHttp |= [schemes containsObject:@"http"];
        handlesHttps |= [schemes containsObject:@"https"];
    }
    if (!handlesHttp || !handlesHttps) return false;

    for (NSDictionary *type in info[@"CFBundleDocumentTypes"]) {
        NSArray<NSString *> *contentTypes = type[@"LSItemContentTypes"];
        NSArray<NSString *> *extensions = type[@"CFBundleTypeExtensions"];
        if ([type[@"CFBundleTypeRole"] isEqualToString:@"Viewer"] &&
            ([contentTypes containsObject:@"public.html"] ||
             [extensions containsObject:@"html"])) {
            return true;
        }
    }
    return false;
}

static NSString *browsefork_icon_data_url(NSWorkspace *workspace, NSString *path) {
    NSImage *icon = [workspace iconForFile:path];
    if (!icon) return nil;
    const NSInteger iconPixels = 384;
    NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc]
        initWithBitmapDataPlanes:NULL
                     pixelsWide:iconPixels
                     pixelsHigh:iconPixels
                  bitsPerSample:8
                samplesPerPixel:4
                       hasAlpha:YES
                       isPlanar:NO
                 colorSpaceName:NSCalibratedRGBColorSpace
                    bytesPerRow:0
                   bitsPerPixel:0];
    if (!bitmap) return nil;
    NSGraphicsContext *context = [NSGraphicsContext graphicsContextWithBitmapImageRep:bitmap];
    if (!context) return nil;
    [NSGraphicsContext saveGraphicsState];
    [NSGraphicsContext setCurrentContext:context];
    context.imageInterpolation = NSImageInterpolationHigh;
    [icon drawInRect:NSMakeRect(0, 0, iconPixels, iconPixels)
           fromRect:NSZeroRect
          operation:NSCompositingOperationSourceOver
           fraction:1.0];
    [context flushGraphics];
    [NSGraphicsContext restoreGraphicsState];
    NSData *png = [bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
    return png ? [@"data:image/png;base64," stringByAppendingString:[png base64EncodedStringWithOptions:0]] : nil;
}

// Updates can register a second copy of the same app in an updater cache.
// Deduplicate by bundle ID, keeping a stable installed path for saved rules.
static NSArray<NSURL *> *browsefork_unique_browser_urls(
    NSArray<NSURL *> *appURLs, NSArray<NSString *> *applicationDirectories, NSURL *ownURL) {
    NSString *ownPath = ownURL.URLByResolvingSymlinksInPath.URLByStandardizingPath.path;
    NSMutableArray<NSURL *> *candidates = [NSMutableArray array];
    for (NSURL *appURL in appURLs) {
        NSURL *canonicalURL = appURL.URLByResolvingSymlinksInPath.URLByStandardizingPath;
        if (!canonicalURL.path || [canonicalURL.path isEqualToString:ownPath]) continue;
        NSBundle *bundle = [NSBundle bundleWithURL:canonicalURL];
        if (bundle && browsefork_is_browser(bundle)) [candidates addObject:canonicalURL];
    }
    BOOL (^isInstalled)(NSURL *) = ^BOOL(NSURL *url) {
        for (NSString *directory in applicationDirectories) {
            NSString *root = [NSURL fileURLWithPath:directory].URLByResolvingSymlinksInPath
                .URLByStandardizingPath.path;
            if ([url.path hasPrefix:[root stringByAppendingString:@"/"]]) return YES;
        }
        return NO;
    };
    [candidates sortUsingComparator:^NSComparisonResult(NSURL *left, NSURL *right) {
        BOOL leftInstalled = isInstalled(left);
        BOOL rightInstalled = isInstalled(right);
        if (leftInstalled != rightInstalled) return leftInstalled ? NSOrderedAscending : NSOrderedDescending;
        // Prefer the app directly in Applications over a nested backup, then use a
        // deterministic tie-breaker rather than Launch Services' enumeration order.
        NSUInteger leftDepth = left.path.pathComponents.count;
        NSUInteger rightDepth = right.path.pathComponents.count;
        if (leftDepth != rightDepth) return leftDepth < rightDepth ? NSOrderedAscending : NSOrderedDescending;
        return [left.path compare:right.path options:NSLiteralSearch];
    }];
    NSMutableSet<NSString *> *seen = [NSMutableSet set];
    NSMutableArray<NSURL *> *unique = [NSMutableArray array];
    for (NSURL *url in candidates) {
        NSString *bundleID = [NSBundle bundleWithURL:url].bundleIdentifier;
        // A bundle without an identifier must not hide an unrelated application.
        NSString *identity = bundleID.length ? [@"bundle:" stringByAppendingString:bundleID]
            : [@"path:" stringByAppendingString:url.path];
        if ([seen containsObject:identity]) continue;
        [seen addObject:identity];
        [unique addObject:url];
    }
    return unique;
}

char *browsefork_list_browsers(void) {
    @autoreleasepool {
        NSWorkspace *workspace = [NSWorkspace sharedWorkspace];
        NSURL *sampleURL = [NSURL URLWithString:@"https://example.com/"];
        NSArray<NSURL *> *appURLs = [workspace URLsForApplicationsToOpenURL:sampleURL];
        NSURL *ownURL = [[NSBundle mainBundle] bundleURL];
        NSMutableArray<NSDictionary *> *browsers = [NSMutableArray array];
        NSArray<NSString *> *applicationDirectories = NSSearchPathForDirectoriesInDomains(
            NSApplicationDirectory, NSAllDomainsMask, YES);
        for (NSURL *appURL in browsefork_unique_browser_urls(appURLs, applicationDirectories, ownURL)) {
            NSString *path = appURL.path;
            NSBundle *bundle = [NSBundle bundleWithURL:appURL];
            NSString *name = [bundle objectForInfoDictionaryKey:@"CFBundleDisplayName"];
            if (!name) name = [bundle objectForInfoDictionaryKey:@"CFBundleName"];
            if (!name) name = [[path lastPathComponent] stringByDeletingPathExtension];
            if (!name) continue;
            NSString *icon = browsefork_icon_data_url(workspace, path);
            [browsers addObject:@{
                @"id": path,
                @"name": name,
                @"icon_data_url": icon ?: [NSNull null]
            }];
        }

        [browsers sortUsingComparator:^NSComparisonResult(NSDictionary *left, NSDictionary *right) {
            return [left[@"name"] localizedCaseInsensitiveCompare:right[@"name"]];
        }];
        NSData *data = [NSJSONSerialization dataWithJSONObject:browsers options:0 error:nil];
        if (!data) return NULL;
        NSString *json = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
        return json ? strdup([json UTF8String]) : NULL;
    }
}

char *browsefork_list_source_apps(void) {
    @autoreleasepool {
        NSMutableArray<NSDictionary *> *apps = [NSMutableArray array];
        NSMutableSet<NSString *> *seen = [NSMutableSet set];
        for (NSRunningApplication *app in [[NSWorkspace sharedWorkspace] runningApplications]) {
            NSString *appID = app.bundleIdentifier;
            if (!appID || [appID isEqualToString:@"com.browsefork.desktop"] ||
                [seen containsObject:appID]) continue;
            [seen addObject:appID];
            [apps addObject:@{ @"id": appID, @"name": app.localizedName ?: appID }];
        }
        [apps sortUsingComparator:^NSComparisonResult(NSDictionary *left, NSDictionary *right) {
            return [left[@"name"] localizedCaseInsensitiveCompare:right[@"name"]];
        }];
        NSData *data = [NSJSONSerialization dataWithJSONObject:apps options:0 error:nil];
        if (!data) return NULL;
        NSString *json = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
        return json ? strdup([json UTF8String]) : NULL;
    }
}

bool browsefork_open_url(const char *app_path, const char *url) {
    @autoreleasepool {
        if (!app_path || !url) return false;
        NSString *path = [NSString stringWithUTF8String:app_path];
        NSString *urlString = [NSString stringWithUTF8String:url];
        if (!path || !urlString) return false;
        NSURL *appURL = [NSURL fileURLWithPath:path];
        NSURL *targetURL = [NSURL URLWithString:urlString];
        if (!targetURL.scheme && [urlString isAbsolutePath]) {
            targetURL = [NSURL fileURLWithPath:urlString];
        }
        if (!targetURL.scheme || ![[NSFileManager defaultManager] fileExistsAtPath:path]) return false;

        [[NSWorkspace sharedWorkspace]
            openURLs:@[targetURL]
            withApplicationAtURL:appURL
            configuration:[NSWorkspaceOpenConfiguration configuration]
            completionHandler:^(NSRunningApplication *application, NSError *error) {
                (void)application;
                if (error) NSLog(@"BrowseFork: failed to open URL: %@", error);
            }];
        return true;
    }
}

void browsefork_free_string(char *value) {
    free(value);
}
