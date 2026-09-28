#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>

typedef void (*BrowseForkURLCallback)(const char *, const char *);
static BrowseForkURLCallback browseforkURLCallback;
static NSMutableArray<NSDictionary *> *browseforkPendingURLs;
static bool browseforkWasDefaultLaunch;

@interface BrowseForkLaunchObserver : NSObject
@end

@implementation BrowseForkLaunchObserver
- (void)didFinishLaunching:(NSNotification *)notification {
    NSNumber *defaultLaunch = notification.userInfo[@"NSApplicationLaunchIsDefaultLaunchKey"];
    browseforkWasDefaultLaunch = [defaultLaunch boolValue];
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
}

bool browsefork_was_default_launch(void) {
    return browseforkWasDefaultLaunch;
}

void browsefork_style_picker_window(void *native_window, bool switcher) {
    NSWindow *window = (__bridge NSWindow *)native_window;
    if (!window) return;
    NSView *content = window.contentView;
    if (!content) return;
    content.wantsLayer = YES;
    content.layer.cornerRadius = switcher ? 24.0 : 0.0;
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

char *browsefork_list_browsers(void) {
    @autoreleasepool {
        NSWorkspace *workspace = [NSWorkspace sharedWorkspace];
        NSURL *sampleURL = [NSURL URLWithString:@"https://example.com/"];
        NSArray<NSURL *> *appURLs = [workspace URLsForApplicationsToOpenURL:sampleURL];
        NSURL *ownURL = [[NSBundle mainBundle] bundleURL];
        NSMutableArray<NSDictionary *> *browsers = [NSMutableArray array];
        NSMutableSet<NSString *> *seen = [NSMutableSet set];

        for (NSURL *appURL in appURLs) {
            NSString *path = [[appURL URLByStandardizingPath] path];
            if (!path || [path isEqualToString:[[ownURL URLByStandardizingPath] path]] ||
                [seen containsObject:path]) {
                continue;
            }
            NSBundle *bundle = [NSBundle bundleWithURL:appURL];
            if (!bundle || !browsefork_is_browser(bundle)) continue;
            NSString *name = [bundle objectForInfoDictionaryKey:@"CFBundleDisplayName"];
            if (!name) name = [bundle objectForInfoDictionaryKey:@"CFBundleName"];
            if (!name) name = [[path lastPathComponent] stringByDeletingPathExtension];
            if (!name) continue;
            [seen addObject:path];
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
