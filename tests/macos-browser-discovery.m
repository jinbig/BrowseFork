// Compile the production adapter in this translation unit to test its selection helper.
#import "../src-tauri/src/platform/macos.m"

static NSURL *makeBrowser(NSString *path, NSString *bundleID) {
    NSString *contents = [path stringByAppendingPathComponent:@"Contents"];
    [[NSFileManager defaultManager] createDirectoryAtPath:contents
        withIntermediateDirectories:YES attributes:nil error:NULL];
    NSMutableDictionary *info = [@{
        @"CFBundleName": @"Brave Browser",
        @"CFBundlePackageType": @"APPL",
        @"CFBundleURLTypes": @[@{@"CFBundleURLSchemes": @[@"http", @"https"]}],
        @"CFBundleDocumentTypes": @[@{@"CFBundleTypeRole": @"Viewer", @"LSItemContentTypes": @[@"public.html"]}]
    } mutableCopy];
    if (bundleID) info[@"CFBundleIdentifier"] = bundleID;
    [info writeToFile:[contents stringByAppendingPathComponent:@"Info.plist"] atomically:YES];
    return [NSURL fileURLWithPath:path];
}

static void check(BOOL condition, NSString *message) {
    if (!condition) { fprintf(stderr, "%s\n", message.UTF8String); exit(1); }
}

int main(void) {
    @autoreleasepool {
        NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSString *applications = [root stringByAppendingPathComponent:@"Applications"];
        NSString *userApplications = [root stringByAppendingPathComponent:@"Home/Applications"];
        NSArray *directories = @[applications, userApplications];
        NSURL *installed = makeBrowser([applications stringByAppendingPathComponent:@"Brave Browser.app"], @"com.brave.Browser");
        NSURL *update = makeBrowser([root stringByAppendingPathComponent:@"Library/Caches/Sparkle/Brave Browser.app"], @"com.brave.Browser");
        NSURL *backup = makeBrowser([applications stringByAppendingPathComponent:@"Backups/Brave Browser.app"], @"com.brave.Browser");
        NSURL *beta = makeBrowser([applications stringByAppendingPathComponent:@"Brave Beta.app"], @"com.brave.Browser.beta");
        NSURL *userInstalled = makeBrowser([userApplications stringByAppendingPathComponent:@"User Browser.app"], @"com.example.UserBrowser");
        NSURL *userUpdate = makeBrowser([root stringByAppendingPathComponent:@"Caches/User Browser.app"], @"com.example.UserBrowser");
        NSArray *unique = browsefork_unique_browser_urls(@[update, installed, backup], directories, nil);
        check(unique.count == 1 && [unique.firstObject isEqual:installed], @"Installed Brave must replace updater and backup copies");
        check([browsefork_unique_browser_urls(@[backup, installed, update], directories, nil) isEqual:unique], @"Enumeration order must not change the chosen path");
        check(browsefork_unique_browser_urls(@[update, installed, beta], directories, nil).count == 2, @"Different editions with the same display name must remain separate");
        NSArray *userUnique = browsefork_unique_browser_urls(@[userUpdate, userInstalled], directories, nil);
        check(userUnique.count == 1 && [userUnique.firstObject isEqual:userInstalled], @"User Applications must outrank a cache copy");
        NSURL *alias = [NSURL fileURLWithPath:[root stringByAppendingPathComponent:@"Brave Alias.app"]];
        [[NSFileManager defaultManager] createSymbolicLinkAtPath:alias.path withDestinationPath:installed.path error:NULL];
        check(browsefork_unique_browser_urls(@[alias, installed], directories, nil).count == 1, @"Symlink aliases must not duplicate a browser");
        NSURL *noID = makeBrowser([applications stringByAppendingPathComponent:@"No ID.app"], nil);
        NSURL *otherNoID = makeBrowser([applications stringByAppendingPathComponent:@"Other No ID.app"], nil);
        check(browsefork_unique_browser_urls(@[noID, otherNoID, noID], directories, nil).count == 2, @"Missing identifiers must fall back to distinct paths");
        NSURL *self = makeBrowser([applications stringByAppendingPathComponent:@"BrowseFork.app"], @"com.browsefork.desktop");
        NSURL *choosy = makeBrowser([applications stringByAppendingPathComponent:@"Choosy.app"], @"com.choosyosx.Choosy");
        NSURL *missing = [NSURL fileURLWithPath:[root stringByAppendingPathComponent:@"Missing.app"]];
        check(browsefork_unique_browser_urls(@[self, choosy, missing, installed], directories, nil).count == 1, @"Self, other choosers, and stale missing apps must be excluded");
        check(browsefork_unique_browser_urls(@[installed], directories, installed).count == 0, @"The current bundle path must be excluded");
        check(browsefork_unique_browser_urls(@[update], directories, nil).count == 1, @"A browser outside Applications must remain usable if it is the only copy");
        [[NSFileManager defaultManager] removeItemAtPath:root error:NULL];
        puts("9 browser discovery regression cases passed");
    }
}
