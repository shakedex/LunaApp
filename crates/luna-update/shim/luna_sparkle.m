// Sparkle 2 for Luna, without linking Sparkle: the framework is loaded at run
// time from the app bundle, so a development build (no bundle) has no updater.
// Signatures follow Sparkle 2.10's SPUStandardUpdaterController.h and
// SPUUpdaterDelegate.h; Sparkle calls delegate methods it finds by selector.

#import <Foundation/Foundation.h>
#import <objc/message.h>
#include <stdlib.h>

typedef void (*luna_sparkle_cb)(int event);

enum { LUNA_FOUND = 0, LUNA_NOT_FOUND = 1, LUNA_SKIPPED = 2 };
// SPUUserUpdateChoice: Skip, Install, Dismiss.
static const NSInteger SPU_CHOICE_SKIP = 0;

static luna_sparkle_cb g_cb;

@interface LunaSparkleDelegate : NSObject
@end

@implementation LunaSparkleDelegate
- (void)updater:(id)updater didFindValidUpdate:(id)item {
    if (g_cb) g_cb(LUNA_FOUND);
}
- (void)updaterDidNotFindUpdate:(id)updater error:(NSError *)error {
    if (g_cb) g_cb(LUNA_NOT_FOUND);
}
- (void)updater:(id)updater userDidMakeChoice:(NSInteger)choice forUpdate:(id)item state:(id)state {
    if (choice == SPU_CHOICE_SKIP && g_cb) g_cb(LUNA_SKIPPED);
}
// LUNA_UPDATE_FEED overrides SUFeedURL for local testing.
- (NSString *)feedURLStringForUpdater:(id)updater {
    const char *feed = getenv("LUNA_UPDATE_FEED");
    return (feed && *feed) ? [NSString stringWithUTF8String:feed] : nil;
}
@end

static LunaSparkleDelegate *g_delegate;
static id g_controller;

// Loads Sparkle.framework at `framework_path` and starts its updater.
// Returns 1 on success, 0 if the framework or its controller is missing.
int luna_sparkle_start(const char *framework_path, luna_sparkle_cb cb) {
    @autoreleasepool {
        NSBundle *bundle = [NSBundle bundleWithPath:[NSString stringWithUTF8String:framework_path]];
        if (!bundle || ![bundle load]) return 0;
        Class controller = NSClassFromString(@"SPUStandardUpdaterController");
        if (!controller) return 0;
        g_cb = cb;
        g_delegate = [LunaSparkleDelegate new];
        SEL init = NSSelectorFromString(@"initWithStartingUpdater:updaterDelegate:userDriverDelegate:");
        id (*send)(id, SEL, BOOL, id, id) = (id (*)(id, SEL, BOOL, id, id))objc_msgSend;
        g_controller = send([controller alloc], init, YES, g_delegate, nil);
        if (!g_controller) return 0;
        // Sparkle's schedule checks once a day; check at every launch too.
        // Sparkle ignores this while its own check is running.
        id (*get)(id, SEL) = (id (*)(id, SEL))objc_msgSend;
        id updater = get(g_controller, NSSelectorFromString(@"updater"));
        void (*check)(id, SEL) = (void (*)(id, SEL))objc_msgSend;
        check(updater, NSSelectorFromString(@"checkForUpdatesInBackground"));
        return 1;
    }
}

// A user-initiated check; Sparkle's window shows the result.
void luna_sparkle_check(void) {
    if (!g_controller) return;
    void (*send)(id, SEL, id) = (void (*)(id, SEL, id))objc_msgSend;
    send(g_controller, NSSelectorFromString(@"checkForUpdates:"), nil);
}
